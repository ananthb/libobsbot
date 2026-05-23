// SPDX-License-Identifier: GPL-3.0-only
//! macOS USB transport via IOKit.
//!
//! Talks to the OBSBOT camera through `IOUSBInterfaceInterface::ControlRequest`
//! on the VideoControl interface. Coexistence note: macOS gives
//! AVFoundation / CoreMediaIO an exclusive claim on a UVC device while
//! it's open in another app, and the kernel UVC driver
//! (`AppleUSBVideoSupport`) routes most class-specific transfers through
//! itself. Our `ControlRequest` calls work as long as no other process
//! has the camera open; the moment FaceTime / Zoom / OBS opens it the
//! transfers start to fail.
//!
//! The OBSBOT vendor SDK works around this on macOS by hooking
//! `CMIOObject` properties on the device, which AVFoundation does forward
//! while it owns the camera. That path needs a separate Transport impl;
//! this one keeps the same IOKit-direct shape as Linux's
//! `uvcvideo`-direct path.

use crate::uvc::UvcGet;
use crate::{Error, Result};

/// IOKit-backed transport. Holds the opened `IOUSBInterfaceInterface`
/// for the camera's VideoControl interface.
pub struct MacosTransport {
    // The actual COM interface pointer + IOKit refcounts will land in
    // the next commit. Keeping this empty for now so the cfg-gated
    // skeleton compiles cleanly on a darwin target.
    _private: (),
}

impl MacosTransport {
    /// Open the OBSBOT camera identified by `info`. Currently
    /// `Err(Unsupported)` - real IOKit opens land in the next commit.
    pub fn open(_info: &crate::discovery::DeviceInfo) -> Result<Self> {
        Err(Error::Unsupported(
            "macos: IOKit transport not yet implemented",
        ))
    }
}

impl super::Transport for MacosTransport {
    fn uvc_set(&self, _entity: u8, _selector: u8, _payload: &[u8]) -> Result<()> {
        Err(Error::Unsupported(
            "macos: IOKit Transport::uvc_set not yet implemented",
        ))
    }

    fn uvc_get(
        &self,
        _req: UvcGet,
        _entity: u8,
        _selector: u8,
        _out: &mut [u8],
    ) -> Result<usize> {
        Err(Error::Unsupported(
            "macos: IOKit Transport::uvc_get not yet implemented",
        ))
    }
}

/// Enumerate OBSBOT cameras via IOKit. Returns empty until the IOKit
/// matching path lands.
pub(crate) fn enumerate() -> Vec<crate::discovery::DeviceInfo> {
    Vec::new()
}
