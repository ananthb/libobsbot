// SPDX-License-Identifier: GPL-3.0-only
//! Transport abstraction for camera control transfers.
//!
//! The transport is the only thing the rest of the library knows about. Every
//! method on [`crate::Device`] is a thin shim that calls into a `Transport`
//! implementation. The default in-tree implementation is [`usb::UsbTransport`]
//! (nusb-backed); test code substitutes a mock.

pub mod usb;

use crate::Result;

/// Camera control transport. One instance per opened device.
pub(crate) trait Transport: Send + Sync {
    /// Issue a UVC class-specific `SET_CUR` on the OBSBOT extension unit.
    fn xu_set(&self, selector: u8, payload: &[u8]) -> Result<()>;

    /// Issue a UVC class-specific `GET_CUR` on the OBSBOT extension unit.
    /// Returns the number of bytes written to `out`.
    fn xu_get(&self, selector: u8, out: &mut [u8]) -> Result<usize>;
}
