// SPDX-License-Identifier: GPL-3.0-only
//! Linux UVC class-specific control transfer transport.
//!
//! Issues UVC class-specific `SET_CUR` and `GET_*` requests via the
//! `uvcvideo` driver's ioctl interface on `/dev/videoN`. This coexists
//! with v4l2 streaming apps (Zoom / OBS / Cheese keep working). See
//! [`super::uvcvideo`] for the V4L2 + UVCIOC dispatch.
//!
//! macOS and Windows transports are planned and will live in sibling
//! modules with the same [`Transport`] surface.

#![cfg(target_os = "linux")]

use std::fs::File;

use crate::discovery::DeviceInfo;
use crate::transport::Transport;
use crate::uvc::UvcGet;
use crate::Result;

/// UVC class-specific `SET_CUR` `bRequest`.
const SET_CUR: u8 = 0x01;

/// Per-device transport. Owns the `/dev/videoN` handle the kernel routes
/// our class-specific transfers through.
pub(crate) struct UsbTransport {
    vendor_id: u16,
    product_id: u16,
    v4l2: File,
}

impl UsbTransport {
    pub(crate) fn open(info: &DeviceInfo) -> Result<Self> {
        let v4l2 = super::uvcvideo::open_for(info.busnum, info.devnum)?;
        tracing::debug!(
            vid = format_args!("{:04x}", info.vendor_id),
            pid = format_args!("{:04x}", info.product_id),
            busnum = info.busnum,
            devnum = info.devnum,
            "opened uvcvideo transport",
        );
        Ok(Self {
            vendor_id: info.vendor_id,
            product_id: info.product_id,
            v4l2,
        })
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
        if let Some(result) = super::uvcvideo::v4l2_set(&self.v4l2, entity, selector, payload) {
            return result;
        }
        // Vendor XU goes through UVCIOC_CTRL_QUERY (requires UVCIOC_CTRL_MAP
        // for each control; that part lands with per-XU pcaps).
        let mut buf = payload.to_vec();
        super::uvcvideo::xu_query(&self.v4l2, entity, selector, SET_CUR, &mut buf).map(|_| ())
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
        if let Some(result) = super::uvcvideo::v4l2_get(&self.v4l2, req, entity, selector, out) {
            return result;
        }
        super::uvcvideo::xu_query(&self.v4l2, entity, selector, req as u8, out)
    }
}
