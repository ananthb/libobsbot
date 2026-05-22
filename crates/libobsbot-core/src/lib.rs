// SPDX-License-Identifier: GPL-3.0-only
//! Rust implementation of the OBSBOT camera SDK.
//!
//! See `README.md` and `CONTRIBUTING.md` at the repository root. v0.0.0 is a
//! skeleton - the public API is unstable and most methods return
//! [`Error::Unsupported`] until the protocol capture work begins.

pub mod device;
pub mod devices;
pub mod discovery;
pub mod error;
pub mod status;
pub mod transport;
pub mod types;
pub(crate) mod uvc;

#[cfg(test)]
mod testing;

pub use device::Device;
pub use discovery::{DeviceInfo, Devices};
pub use error::{Error, Result};
pub use status::{Event, EventReceiver};
pub use types::{
    AiMode, AutoFramingMode, FovType, MediaMode, ProductType, Status, WdrMode, WhiteBalanceMode,
};
