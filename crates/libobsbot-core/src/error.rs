// SPDX-License-Identifier: GPL-3.0-only
//! Error type for libobsbot-core.

use thiserror::Error;

/// Result type used throughout libobsbot-core.
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// Errors produced by libobsbot.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// USB transport failure.
    #[error("usb: {0}")]
    Usb(String),

    /// Operation timed out before the camera responded.
    #[error("timeout")]
    Timeout,

    /// Camera returned a response that did not match the expected shape.
    #[error("bad response for selector {selector:#04x}: {bytes:02x?}")]
    BadResponse {
        /// XU selector the response was for.
        selector: u8,
        /// Raw response bytes for debugging.
        bytes: Vec<u8>,
    },

    /// Method is not supported on this platform or for this device.
    #[error("unsupported: {0}")]
    Unsupported(&'static str),

    /// No matching device was found.
    #[error("not found")]
    NotFound,

    /// Argument value lies outside the camera's accepted range.
    #[error("out of range")]
    OutOfRange,

    /// Camera firmware is older than the minimum supported revision.
    #[error("firmware {observed} is older than required {required}")]
    FirmwareUnsupported {
        /// Firmware version reported by the camera.
        observed: String,
        /// Minimum firmware version this build supports.
        required: String,
    },
}
