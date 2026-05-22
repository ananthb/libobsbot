// SPDX-License-Identifier: GPL-3.0-only
//! UVC class-specific control transfer transport.
//!
//! Two code paths share the same [`Transport`] surface:
//!
//! 1. **Linux, when `uvcvideo` owns the camera** — ioctl
//!    `UVCIOC_CTRL_QUERY` on `/dev/videoN`. Coexists with v4l2 streaming
//!    (Zoom / OBS / Cheese keep working).
//! 2. **Otherwise** — direct `nusb` control transfers on endpoint 0.
//!    Used on macOS and Windows, and on Linux when `uvcvideo` is not
//!    bound to the device. The kernel auto-claims the interface for us.
//!
//! Both paths converge on the same UVC class-specific request format —
//! `(entity, selector, bRequest)` plus payload.

use std::time::Duration;

use nusb::transfer::{ControlIn, ControlOut, ControlType, Recipient};
use nusb::Device as NusbDevice;
use nusb::MaybeFuture;

use crate::transport::Transport;
use crate::uvc::UvcGet;
use crate::{Error, Result};

/// UVC class-specific `SET_CUR` `bRequest`.
const SET_CUR: u8 = 0x01;

/// Timeout for synchronous control transfers.
const CONTROL_TIMEOUT: Duration = Duration::from_millis(1500);

/// USB transport for an opened camera.
pub(crate) struct UsbTransport {
    device: NusbDevice,
    vendor_id: u16,
    product_id: u16,
    video_control_interface: u8,
    /// `/dev/videoN` handle for `UVCIOC_CTRL_QUERY`, when found at open time.
    #[cfg(target_os = "linux")]
    v4l2: Option<std::fs::File>,
}

impl UsbTransport {
    /// Open the given nusb device and prepare it for UVC class control transfers.
    #[allow(clippy::needless_pass_by_value)] // nusb::DeviceInfo::open consumes self
    pub(crate) fn open(info: nusb::DeviceInfo, video_control_interface: u8) -> Result<Self> {
        let vendor_id = info.vendor_id();
        let product_id = info.product_id();
        #[cfg(target_os = "linux")]
        let v4l2 = {
            let busnum = info.busnum();
            let devnum = info.device_address();
            match super::uvcvideo::open_for(busnum, devnum) {
                Ok(f) => {
                    tracing::debug!(busnum, devnum, "opened /dev/videoN for UVCIOC_CTRL_QUERY");
                    Some(f)
                }
                Err(e) => {
                    tracing::debug!(
                        busnum,
                        devnum,
                        "no matching /dev/videoN ({e}); falling back to nusb direct"
                    );
                    None
                }
            }
        };
        let device = info
            .open()
            .wait()
            .map_err(|e| Error::Usb(format!("open: {e}")))?;
        Ok(Self {
            device,
            vendor_id,
            product_id,
            video_control_interface,
            #[cfg(target_os = "linux")]
            v4l2,
        })
    }

    fn w_index(&self, entity: u8) -> u16 {
        (u16::from(entity) << 8) | u16::from(self.video_control_interface)
    }

    fn nusb_set(&self, entity: u8, selector: u8, payload: &[u8]) -> Result<()> {
        let req = ControlOut {
            control_type: ControlType::Class,
            recipient: Recipient::Interface,
            request: SET_CUR,
            value: u16::from(selector) << 8,
            index: self.w_index(entity),
            data: payload,
        };
        self.device
            .control_out(req, CONTROL_TIMEOUT)
            .wait()
            .map_err(|e| Error::Usb(format!("SET_CUR entity={entity} sel={selector:#04x}: {e}")))
    }

    fn nusb_get(&self, req: UvcGet, entity: u8, selector: u8, out: &mut [u8]) -> Result<usize> {
        let length = u16::try_from(out.len())
            .map_err(|_| Error::Usb(format!("uvc_get buffer too large: {}", out.len())))?;
        let xfer = ControlIn {
            control_type: ControlType::Class,
            recipient: Recipient::Interface,
            request: req as u8,
            value: u16::from(selector) << 8,
            index: self.w_index(entity),
            length,
        };
        let bytes = self
            .device
            .control_in(xfer, CONTROL_TIMEOUT)
            .wait()
            .map_err(|e| Error::Usb(format!("{req:?} entity={entity} sel={selector:#04x}: {e}")))?;
        let n = bytes.len().min(out.len());
        out[..n].copy_from_slice(&bytes[..n]);
        Ok(n)
    }
}

impl Transport for UsbTransport {
    fn uvc_set(&self, entity: u8, selector: u8, payload: &[u8]) -> Result<()> {
        tracing::trace!(
            vid = format_args!("{:04x}", self.vendor_id),
            pid = format_args!("{:04x}", self.product_id),
            entity,
            selector = format_args!("{selector:#04x}"),
            len = payload.len(),
            "uvc_set",
        );
        #[cfg(target_os = "linux")]
        if let Some(fd) = self.v4l2.as_ref() {
            if let Some(result) = super::uvcvideo::v4l2_set(fd, entity, selector, payload) {
                return result;
            }
            // Vendor XU goes through UVCIOC_CTRL_QUERY (requires UVCIOC_CTRL_MAP
            // for each control; that part lands with per-XU pcaps).
            let mut buf = payload.to_vec();
            return super::uvcvideo::xu_query(fd, entity, selector, SET_CUR, &mut buf).map(|_| ());
        }
        self.nusb_set(entity, selector, payload)
    }

    fn uvc_get(&self, req: UvcGet, entity: u8, selector: u8, out: &mut [u8]) -> Result<usize> {
        tracing::trace!(
            vid = format_args!("{:04x}", self.vendor_id),
            pid = format_args!("{:04x}", self.product_id),
            entity,
            selector = format_args!("{selector:#04x}"),
            req = ?req,
            length = out.len(),
            "uvc_get",
        );
        #[cfg(target_os = "linux")]
        if let Some(fd) = self.v4l2.as_ref() {
            if let Some(result) = super::uvcvideo::v4l2_get(fd, req, entity, selector, out) {
                return result;
            }
            return super::uvcvideo::xu_query(fd, entity, selector, req as u8, out);
        }
        self.nusb_get(req, entity, selector, out)
    }
}
