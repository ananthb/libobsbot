// SPDX-License-Identifier: GPL-3.0-only
//! nusb-backed UVC class-specific control transfer transport.
//!
//! Issues UVC class-specific `SET_CUR` and `GET_*` requests on endpoint 0
//! against the opened nusb device. On Linux this routes through usbfs's
//! `USBDEVFS_CONTROL`, which works even while the `uvcvideo` kernel driver
//! holds the device — Zoom / OBS / Cheese can keep streaming.

use std::time::Duration;

use nusb::transfer::{ControlIn, ControlOut, ControlType, Recipient};
use nusb::Device as NusbDevice;
use nusb::MaybeFuture;

use crate::transport::Transport;
use crate::uvc::UvcGet;
use crate::{Error, Result};

/// UVC class-specific `SET_CUR` `bRequest`.
const SET_CUR: u8 = 0x01;

/// Timeout for synchronous control transfers. OBSBOT's libdev.so uses
/// ~1 s in observed captures; we keep some margin.
const CONTROL_TIMEOUT: Duration = Duration::from_millis(1500);

/// USB transport for an opened camera.
pub(crate) struct UsbTransport {
    device: NusbDevice,
    vendor_id: u16,
    product_id: u16,
    video_control_interface: u8,
}

impl UsbTransport {
    /// Open the given nusb device and prepare it for UVC class control transfers.
    #[allow(clippy::needless_pass_by_value)] // nusb::DeviceInfo::open consumes self
    pub(crate) fn open(info: nusb::DeviceInfo, video_control_interface: u8) -> Result<Self> {
        let vendor_id = info.vendor_id();
        let product_id = info.product_id();
        let device = info
            .open()
            .wait()
            .map_err(|e| Error::Usb(format!("open: {e}")))?;
        Ok(Self {
            device,
            vendor_id,
            product_id,
            video_control_interface,
        })
    }

    fn w_index(&self, entity: u8) -> u16 {
        (u16::from(entity) << 8) | u16::from(self.video_control_interface)
    }
}

impl Transport for UsbTransport {
    fn uvc_set(&self, entity: u8, selector: u8, payload: &[u8]) -> Result<()> {
        let req = ControlOut {
            control_type: ControlType::Class,
            recipient: Recipient::Interface,
            request: SET_CUR,
            value: u16::from(selector) << 8,
            index: self.w_index(entity),
            data: payload,
        };
        tracing::trace!(
            vid = format_args!("{:04x}", self.vendor_id),
            pid = format_args!("{:04x}", self.product_id),
            entity,
            selector = format_args!("{selector:#04x}"),
            len = payload.len(),
            "uvc_set",
        );
        self.device
            .control_out(req, CONTROL_TIMEOUT)
            .wait()
            .map_err(|e| Error::Usb(format!("SET_CUR entity={entity} sel={selector:#04x}: {e}")))
    }

    fn uvc_get(&self, req: UvcGet, entity: u8, selector: u8, out: &mut [u8]) -> Result<usize> {
        let length = u16::try_from(out.len())
            .map_err(|_| Error::Usb(format!("uvc_get buffer too large: {}", out.len())))?;
        let req_byte = req as u8;
        let xfer = ControlIn {
            control_type: ControlType::Class,
            recipient: Recipient::Interface,
            request: req_byte,
            value: u16::from(selector) << 8,
            index: self.w_index(entity),
            length,
        };
        tracing::trace!(
            vid = format_args!("{:04x}", self.vendor_id),
            pid = format_args!("{:04x}", self.product_id),
            entity,
            selector = format_args!("{selector:#04x}"),
            req = ?req,
            length,
            "uvc_get",
        );
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
