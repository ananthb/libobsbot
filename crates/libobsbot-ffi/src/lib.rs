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

use libobsbot_core::{
    AiMode, AutoFramingMode, Cadence, Device, Devices, Error, FovType, MediaMode, WdrMode,
    WhiteBalanceMode,
};

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
    with_device(handle, |d| d.set_brightness(value))
}

/// Read camera brightness into `*out_value`.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_brightness(
    handle: *mut ObsbotDevice,
    out_value: *mut i32,
) -> c_int {
    read_into(handle, out_value, Device::brightness)
}

/// Set camera contrast. Returns one of the `OBSBOT_*` codes.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_contrast(
    handle: *mut ObsbotDevice,
    value: i32,
) -> c_int {
    with_device(handle, |d| d.set_contrast(value))
}

/// Read camera contrast into `*out_value`.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_contrast(
    handle: *mut ObsbotDevice,
    out_value: *mut i32,
) -> c_int {
    read_into(handle, out_value, Device::contrast)
}

/// Set camera saturation. Returns one of the `OBSBOT_*` codes.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_saturation(
    handle: *mut ObsbotDevice,
    value: i32,
) -> c_int {
    with_device(handle, |d| d.set_saturation(value))
}

/// Read camera saturation into `*out_value`.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_saturation(
    handle: *mut ObsbotDevice,
    out_value: *mut i32,
) -> c_int {
    read_into(handle, out_value, Device::saturation)
}

/// Set pan + tilt as normalised values in -1.0..=1.0.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_pan_tilt(
    handle: *mut ObsbotDevice,
    pan: f32,
    tilt: f32,
) -> c_int {
    with_device(handle, |d| d.set_pan_tilt(pan, tilt))
}

/// Set zoom as a u16-clamped objective focal length.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_zoom(handle: *mut ObsbotDevice, zoom: f32) -> c_int {
    with_device(handle, |d| d.set_zoom(zoom))
}

/// Set focus as a u16-clamped distance value.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_focus(handle: *mut ObsbotDevice, focus: f32) -> c_int {
    with_device(handle, |d| d.set_focus(focus))
}

/// White-balance mode: 0 = Auto, 1 = Manual. `kelvin` is honoured only
/// for Manual; pass 0 with Auto.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_white_balance(
    handle: *mut ObsbotDevice,
    mode: c_int,
    kelvin: u16,
) -> c_int {
    let Some(wb) = wb_from_int(mode) else {
        return OBSBOT_ERR_OUT_OF_RANGE;
    };
    let kelvin_opt = if matches!(wb, WhiteBalanceMode::Manual) {
        Some(kelvin)
    } else {
        None
    };
    with_device(handle, |d| d.set_white_balance(wb, kelvin_opt))
}

/// HDR mode: 0 = Off, 1 = `Dol2To1`.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_wdr(handle: *mut ObsbotDevice, mode: c_int) -> c_int {
    let Some(m) = wdr_from_int(mode) else {
        return OBSBOT_ERR_OUT_OF_RANGE;
    };
    with_device(handle, |d| d.set_wdr(m))
}

/// FOV preset: 0 = Wide, 1 = Medium, 2 = Narrow.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_fov(handle: *mut ObsbotDevice, fov: c_int) -> c_int {
    let Some(f) = fov_from_int(fov) else {
        return OBSBOT_ERR_OUT_OF_RANGE;
    };
    with_device(handle, |d| d.set_fov(f))
}

/// Face-AE toggle: non-zero on, zero off.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_face_ae(handle: *mut ObsbotDevice, on: c_int) -> c_int {
    with_device(handle, |d| d.set_face_ae(on != 0))
}

/// Face-focus toggle: non-zero on, zero off.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_face_focus(
    handle: *mut ObsbotDevice,
    on: c_int,
) -> c_int {
    with_device(handle, |d| d.set_face_focus(on != 0))
}

/// Media mode: 0 = Normal, 1 = Background, 2 = `AutoFrame`.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_media_mode(
    handle: *mut ObsbotDevice,
    mode: c_int,
) -> c_int {
    let Some(m) = media_from_int(mode) else {
        return OBSBOT_ERR_OUT_OF_RANGE;
    };
    with_device(handle, |d| d.set_media_mode(m))
}

/// Auto-framing sub-mode: 0 = Group, 1 = `SingleCloseUp`, 2 = `SingleUpperBody`.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_auto_framing(
    handle: *mut ObsbotDevice,
    mode: c_int,
) -> c_int {
    let Some(m) = framing_from_int(mode) else {
        return OBSBOT_ERR_OUT_OF_RANGE;
    };
    with_device(handle, |d| d.set_auto_framing(m))
}

/// AI master mode: 0 = None, 1 = Group, 2 = Human, 3 = Hand,
/// 4 = `WhiteBoard`, 5 = Desk.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_ai_mode(
    handle: *mut ObsbotDevice,
    mode: c_int,
) -> c_int {
    let Some(m) = ai_from_int(mode) else {
        return OBSBOT_ERR_OUT_OF_RANGE;
    };
    with_device(handle, |d| d.set_ai_mode(m))
}

/// Status poller cadence: 0 = Slow (2.5 s), 1 = Fast (25 ms).
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_status_cadence(
    handle: *mut ObsbotDevice,
    cadence: c_int,
) -> c_int {
    let Some(c) = cadence_from_int(cadence) else {
        return OBSBOT_ERR_OUT_OF_RANGE;
    };
    if handle.is_null() {
        return OBSBOT_ERR_NOT_FOUND;
    }
    (*handle).0.set_status_cadence(c);
    OBSBOT_OK
}

/// Read the camera-reported firmware version into `out_buf` as a
/// NUL-terminated string. `buf_len` must include space for the NUL.
/// Returns `OBSBOT_ERR_OUT_OF_RANGE` if the buffer is too small.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_firmware(
    handle: *mut ObsbotDevice,
    out_buf: *mut c_char,
    buf_len: usize,
) -> c_int {
    copy_string_out(handle, out_buf, buf_len, Device::firmware_from_camera)
}

/// Read the camera-reported serial number into `out_buf` as a
/// NUL-terminated string. Same buffer-size contract as
/// [`obsbot_device_firmware`].
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_serial(
    handle: *mut ObsbotDevice,
    out_buf: *mut c_char,
    buf_len: usize,
) -> c_int {
    copy_string_out(handle, out_buf, buf_len, Device::serial_from_camera)
}

// ---- helpers --------------------------------------------------------------

unsafe fn with_device<F: FnOnce(&Device) -> Result<(), Error>>(
    handle: *mut ObsbotDevice,
    f: F,
) -> c_int {
    if handle.is_null() {
        return OBSBOT_ERR_NOT_FOUND;
    }
    match f(&(*handle).0) {
        Ok(()) => OBSBOT_OK,
        Err(e) => map_error(&e),
    }
}

unsafe fn read_into<T: Copy, F: FnOnce(&Device) -> Result<T, Error>>(
    handle: *mut ObsbotDevice,
    out: *mut T,
    f: F,
) -> c_int {
    if handle.is_null() || out.is_null() {
        return OBSBOT_ERR_NOT_FOUND;
    }
    match f(&(*handle).0) {
        Ok(v) => {
            *out = v;
            OBSBOT_OK
        }
        Err(e) => map_error(&e),
    }
}

unsafe fn copy_string_out<F: FnOnce(&Device) -> Result<String, Error>>(
    handle: *mut ObsbotDevice,
    out_buf: *mut c_char,
    buf_len: usize,
    f: F,
) -> c_int {
    if handle.is_null() || out_buf.is_null() || buf_len == 0 {
        return OBSBOT_ERR_NOT_FOUND;
    }
    let s = match f(&(*handle).0) {
        Ok(s) => s,
        Err(e) => return map_error(&e),
    };
    let bytes = s.as_bytes();
    if bytes.len() + 1 > buf_len {
        return OBSBOT_ERR_OUT_OF_RANGE;
    }
    ptr::copy_nonoverlapping(bytes.as_ptr().cast::<c_char>(), out_buf, bytes.len());
    *out_buf.add(bytes.len()) = 0;
    OBSBOT_OK
}

const fn wb_from_int(v: c_int) -> Option<WhiteBalanceMode> {
    match v {
        0 => Some(WhiteBalanceMode::Auto),
        1 => Some(WhiteBalanceMode::Manual),
        _ => None,
    }
}

const fn wdr_from_int(v: c_int) -> Option<WdrMode> {
    match v {
        0 => Some(WdrMode::Off),
        1 => Some(WdrMode::Dol2To1),
        _ => None,
    }
}

const fn fov_from_int(v: c_int) -> Option<FovType> {
    match v {
        0 => Some(FovType::Wide),
        1 => Some(FovType::Medium),
        2 => Some(FovType::Narrow),
        _ => None,
    }
}

const fn media_from_int(v: c_int) -> Option<MediaMode> {
    match v {
        0 => Some(MediaMode::Normal),
        1 => Some(MediaMode::Background),
        2 => Some(MediaMode::AutoFrame),
        _ => None,
    }
}

const fn framing_from_int(v: c_int) -> Option<AutoFramingMode> {
    match v {
        0 => Some(AutoFramingMode::Group),
        1 => Some(AutoFramingMode::SingleCloseUp),
        2 => Some(AutoFramingMode::SingleUpperBody),
        _ => None,
    }
}

const fn ai_from_int(v: c_int) -> Option<AiMode> {
    match v {
        0 => Some(AiMode::None),
        1 => Some(AiMode::Group),
        2 => Some(AiMode::Human),
        3 => Some(AiMode::Hand),
        4 => Some(AiMode::WhiteBoard),
        5 => Some(AiMode::Desk),
        _ => None,
    }
}

const fn cadence_from_int(v: c_int) -> Option<Cadence> {
    match v {
        0 => Some(Cadence::Slow),
        1 => Some(Cadence::Fast),
        _ => None,
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
        let rc = unsafe { obsbot_devices_open_first(ptr::null_mut(), &raw mut out) };
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
