// SPDX-License-Identifier: GPL-3.0-only
//! nusb-backed UVC Extension Unit transport.
//!
//! v0.0.0 wires up the type but does not yet issue real control transfers —
//! both [`Transport::xu_set`] and [`Transport::xu_get`] return
//! [`Error::Unsupported`]. The actual `control_in`/`control_out` calls land
//! in M2/M3 once the first capture confirms the XU GUID, entity id, and
//! `VideoControl` interface number for the OBSBOT Meet 2.

use crate::{Error, Result};

use super::Transport;

/// USB transport for an opened camera.
pub(crate) struct UsbTransport {
    /// USB vendor id of the opened device. Used for diagnostics in error
    /// messages until the real transport lands.
    vendor_id: u16,
    /// USB product id of the opened device.
    product_id: u16,
}

impl UsbTransport {
    pub(crate) fn new(vendor_id: u16, product_id: u16) -> Self {
        Self {
            vendor_id,
            product_id,
        }
    }
}

impl Transport for UsbTransport {
    fn xu_set(&self, _selector: u8, _payload: &[u8]) -> Result<()> {
        tracing::debug!(
            vid = format_args!("{:04x}", self.vendor_id),
            pid = format_args!("{:04x}", self.product_id),
            "xu_set called before transport is implemented",
        );
        Err(Error::Unsupported(
            "xu_set: protocol capture pending (see docs/protocol/meet2/)",
        ))
    }

    fn xu_get(&self, _selector: u8, _out: &mut [u8]) -> Result<usize> {
        tracing::debug!(
            vid = format_args!("{:04x}", self.vendor_id),
            pid = format_args!("{:04x}", self.product_id),
            "xu_get called before transport is implemented",
        );
        Err(Error::Unsupported(
            "xu_get: protocol capture pending (see docs/protocol/meet2/)",
        ))
    }
}
