// SPDX-License-Identifier: GPL-3.0-only
//! nusb-backed UVC class-specific control transfer transport.
//!
//! v0.0.0 carries the per-device identifiers but does not yet issue real
//! control transfers — [`Transport::uvc_set`] and [`Transport::uvc_get`]
//! return [`Error::Unsupported`]. The actual `control_in`/`control_out`
//! calls land once the first per-method pcap confirms one selector end-to-end.

use crate::transport::Transport;
use crate::uvc::UvcGet;
use crate::{Error, Result};

/// USB transport for an opened camera.
pub(crate) struct UsbTransport {
    vendor_id: u16,
    product_id: u16,
    video_control_interface: u8,
}

impl UsbTransport {
    pub(crate) fn new(vendor_id: u16, product_id: u16, video_control_interface: u8) -> Self {
        Self {
            vendor_id,
            product_id,
            video_control_interface,
        }
    }
}

impl Transport for UsbTransport {
    fn uvc_set(&self, entity: u8, selector: u8, _payload: &[u8]) -> Result<()> {
        tracing::debug!(
            vid = format_args!("{:04x}", self.vendor_id),
            pid = format_args!("{:04x}", self.product_id),
            iface = self.video_control_interface,
            entity,
            selector = format_args!("{selector:#04x}"),
            "uvc_set called before transport is implemented",
        );
        Err(Error::Unsupported(
            "uvc_set: real control_out pending (see docs/protocol/meet2/)",
        ))
    }

    fn uvc_get(&self, req: UvcGet, entity: u8, selector: u8, _out: &mut [u8]) -> Result<usize> {
        tracing::debug!(
            vid = format_args!("{:04x}", self.vendor_id),
            pid = format_args!("{:04x}", self.product_id),
            iface = self.video_control_interface,
            entity,
            selector = format_args!("{selector:#04x}"),
            req = ?req,
            "uvc_get called before transport is implemented",
        );
        Err(Error::Unsupported(
            "uvc_get: real control_in pending (see docs/protocol/meet2/)",
        ))
    }
}
