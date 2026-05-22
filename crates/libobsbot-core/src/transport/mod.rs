// SPDX-License-Identifier: GPL-3.0-only
//! Transport abstraction for camera control transfers.
//!
//! Every method on [`crate::Device`] is a thin shim that calls into a
//! `Transport` implementation. The default in-tree implementation is
//! [`usb::UsbTransport`] (ioctls on `/dev/videoN` via the `uvcvideo` driver
//! on Linux); test code substitutes a mock.
//!
//! The trait expresses USB Video Class class-specific control transfers
//! identified by `(entity_id, selector)`. Higher layers know which entity
//! they target — Camera Terminal, Processing Unit, or vendor Extension Unit.

#[cfg(target_os = "linux")]
pub mod usb;
#[cfg(target_os = "linux")]
pub(crate) mod uvcvideo;

use crate::uvc::UvcGet;
use crate::Result;

/// Camera control transport. One instance per opened device.
pub(crate) trait Transport: Send + Sync {
    /// Issue a UVC class-specific `SET_CUR` on `(entity, selector)`.
    fn uvc_set(&self, entity: u8, selector: u8, payload: &[u8]) -> Result<()>;

    /// Issue a UVC class-specific `GET_*` on `(entity, selector)` and write
    /// the response into `out`. Returns the number of bytes written.
    fn uvc_get(&self, req: UvcGet, entity: u8, selector: u8, out: &mut [u8]) -> Result<usize>;
}
