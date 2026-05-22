// SPDX-License-Identifier: GPL-3.0-only
//! C ABI for libobsbot.
//!
//! All exported functions are prefixed `obsbot_`. Handles cross the FFI
//! boundary as opaque pointers. Errors are returned as `int32_t`: `0` on
//! success, negative on failure.

#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::missing_safety_doc)]

use core::ffi::{c_char, c_int};
use std::ptr;

use libobsbot_core::{Device, Devices, Error};

// ---- error codes -----------------------------------------------------------

/// Success.
pub const OBSBOT_OK: c_int = 0;
/// Generic / USB error.
pub const OBSBOT_ERR_USB: c_int = -1;
/// No matching device.
pub const OBSBOT_ERR_NOT_FOUND: c_int = -2;
/// Timeout while waiting for a camera response.
pub const OBSBOT_ERR_TIMEOUT: c_int = -3;
/// Operation unsupported on this platform or device.
pub const OBSBOT_ERR_UNSUPPORTED: c_int = -4;
/// Argument out of accepted range.
pub const OBSBOT_ERR_OUT_OF_RANGE: c_int = -5;
/// Camera firmware too old.
pub const OBSBOT_ERR_FIRMWARE: c_int = -6;
/// Camera returned a malformed response.
pub const OBSBOT_ERR_BAD_RESPONSE: c_int = -7;

fn map_error(err: &Error) -> c_int {
    match err {
        Error::NotFound => OBSBOT_ERR_NOT_FOUND,
        Error::Timeout => OBSBOT_ERR_TIMEOUT,
        Error::Unsupported(_) => OBSBOT_ERR_UNSUPPORTED,
        Error::OutOfRange => OBSBOT_ERR_OUT_OF_RANGE,
        Error::FirmwareUnsupported { .. } => OBSBOT_ERR_FIRMWARE,
        Error::BadResponse { .. } => OBSBOT_ERR_BAD_RESPONSE,
        // Error::Usb and any future #[non_exhaustive] variants all map to USB.
        _ => OBSBOT_ERR_USB,
    }
}

// ---- opaque handles --------------------------------------------------------

/// Opaque handle to the hot-plug watcher / device registry.
pub struct ObsbotDevices(Devices);

/// Opaque handle to an opened OBSBOT camera.
pub struct ObsbotDevice(Device);

/// Create a new `ObsbotDevices` registry. Returns NULL on failure.
#[no_mangle]
pub extern "C" fn obsbot_devices_new() -> *mut ObsbotDevices {
    match Devices::new() {
        Ok(d) => Box::into_raw(Box::new(ObsbotDevices(d))),
        Err(_) => ptr::null_mut(),
    }
}

/// Free an `ObsbotDevices` registry previously returned by
/// [`obsbot_devices_new`]. Passing NULL is a no-op.
#[no_mangle]
pub unsafe extern "C" fn obsbot_devices_free(handle: *mut ObsbotDevices) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}

/// Number of OBSBOT cameras currently connected.
#[no_mangle]
pub unsafe extern "C" fn obsbot_devices_count(handle: *mut ObsbotDevices) -> c_int {
    if handle.is_null() {
        return OBSBOT_ERR_NOT_FOUND;
    }
    let devices = &(*handle).0;
    c_int::try_from(devices.list().len()).unwrap_or(c_int::MAX)
}

/// Open the first connected OBSBOT camera. Writes the resulting handle to
/// `*out_device`. Returns one of the `OBSBOT_*` codes.
#[no_mangle]
pub unsafe extern "C" fn obsbot_devices_open_first(
    handle: *mut ObsbotDevices,
    out_device: *mut *mut ObsbotDevice,
) -> c_int {
    if handle.is_null() || out_device.is_null() {
        return OBSBOT_ERR_NOT_FOUND;
    }
    let devices = &(*handle).0;
    let Some(info) = devices.list().into_iter().next() else {
        return OBSBOT_ERR_NOT_FOUND;
    };
    match devices.open(&info) {
        Ok(dev) => {
            *out_device = Box::into_raw(Box::new(ObsbotDevice(dev)));
            OBSBOT_OK
        }
        Err(e) => map_error(&e),
    }
}

/// Free an `ObsbotDevice` previously returned by [`obsbot_devices_open_first`].
/// Passing NULL is a no-op.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_close(handle: *mut ObsbotDevice) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}

/// Set camera brightness. Returns one of the `OBSBOT_*` codes.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_brightness(
    handle: *mut ObsbotDevice,
    value: i32,
) -> c_int {
    if handle.is_null() {
        return OBSBOT_ERR_NOT_FOUND;
    }
    let device = &(*handle).0;
    match device.set_brightness(value) {
        Ok(()) => OBSBOT_OK,
        Err(e) => map_error(&e),
    }
}

// ---- version --------------------------------------------------------------

/// Return the libobsbot version string (NUL-terminated, static lifetime).
#[no_mangle]
pub extern "C" fn obsbot_version() -> *const c_char {
    static VERSION: &[u8] = concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes();
    VERSION.as_ptr().cast::<c_char>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_non_empty() {
        unsafe {
            let p = obsbot_version();
            assert!(!p.is_null());
            let s = std::ffi::CStr::from_ptr(p).to_str().unwrap();
            assert!(!s.is_empty());
        }
    }

    #[test]
    fn devices_new_free_is_safe() {
        let h = obsbot_devices_new();
        assert!(!h.is_null());
        unsafe { obsbot_devices_free(h) };
        unsafe { obsbot_devices_free(ptr::null_mut()) };
    }

    #[test]
    fn count_is_non_negative_or_not_found() {
        let h = obsbot_devices_new();
        let n = unsafe { obsbot_devices_count(h) };
        unsafe { obsbot_devices_free(h) };
        assert!(n >= 0 || n == OBSBOT_ERR_NOT_FOUND);
    }

    #[test]
    fn null_handles_report_not_found() {
        let mut out: *mut ObsbotDevice = ptr::null_mut();
        let rc = unsafe { obsbot_devices_open_first(ptr::null_mut(), &mut out) };
        assert_eq!(rc, OBSBOT_ERR_NOT_FOUND);
        let rc = unsafe { obsbot_device_set_brightness(ptr::null_mut(), 0) };
        assert_eq!(rc, OBSBOT_ERR_NOT_FOUND);
    }

    #[test]
    fn brightness_through_mocked_path_maps_to_unsupported() {
        // Without hardware, open_first returns NOT_FOUND. The brightness call
        // path is exercised by the unit test in libobsbot-core. This test
        // pins the error mapping for Unsupported, which is what
        // device.set_brightness will return once a device is real.
        assert_eq!(map_error(&Error::Unsupported("x")), OBSBOT_ERR_UNSUPPORTED);
        assert_eq!(map_error(&Error::NotFound), OBSBOT_ERR_NOT_FOUND);
    }
}
