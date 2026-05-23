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
    AeMode, AiMode, AntiFlicker, AutoFramingMode, Cadence, Device, Devices, Error, FovType,
    MediaMode, WdrMode, WhiteBalanceMode,
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

/// Event kind delivered by [`obsbot_devices_poll_event`].
pub const OBSBOT_EVENT_DEVICE_ADDED: c_int = 1;
/// See [`OBSBOT_EVENT_DEVICE_ADDED`].
pub const OBSBOT_EVENT_DEVICE_REMOVED: c_int = 2;
/// See [`OBSBOT_EVENT_DEVICE_ADDED`].
pub const OBSBOT_EVENT_STATUS: c_int = 3;

/// One event delivered by [`obsbot_devices_poll_event`]. For Added /
/// Removed events the `status` fields are zeroed; for Status events
/// every field is filled exactly as in
/// [`obsbot_device_status`].
#[repr(C)]
pub struct ObsbotEvent {
    /// One of the `OBSBOT_EVENT_*` constants.
    pub kind: c_int,
    /// Camera serial associated with the event. NUL-terminated.
    pub serial: [c_char; OBSBOT_STR_MAX],
    /// For Status events: the status snapshot. Zeroed otherwise.
    pub status: ObsbotStatus,
}

/// Pull the next event off the registry's queue. `timeout_ms` < 0
/// blocks indefinitely, 0 returns immediately if nothing is ready,
/// > 0 waits up to that many milliseconds.
///
/// Returns `OBSBOT_OK` on success, `OBSBOT_ERR_TIMEOUT` when no event
/// arrived within `timeout_ms`, and `OBSBOT_ERR_NOT_FOUND` for invalid
/// arguments or a closed channel.
#[no_mangle]
pub unsafe extern "C" fn obsbot_devices_poll_event(
    handle: *mut ObsbotDevices,
    out_event: *mut ObsbotEvent,
    timeout_ms: i32,
) -> c_int {
    if handle.is_null() || out_event.is_null() {
        return OBSBOT_ERR_NOT_FOUND;
    }
    let rx = (*handle).0.events();
    let received = match timeout_ms.cmp(&0) {
        std::cmp::Ordering::Less => rx.recv().ok(),
        std::cmp::Ordering::Equal => rx.try_recv().ok(),
        std::cmp::Ordering::Greater => {
            let dur = std::time::Duration::from_millis(timeout_ms.unsigned_abs().into());
            rx.recv_timeout(dur).ok()
        }
    };
    let Some(ev) = received else {
        return OBSBOT_ERR_TIMEOUT;
    };
    ptr::write_bytes(out_event, 0, 1);
    match ev {
        libobsbot_core::Event::DeviceAdded { serial } => {
            (*out_event).kind = OBSBOT_EVENT_DEVICE_ADDED;
            copy_into_buf((*out_event).serial.as_mut_ptr(), OBSBOT_STR_MAX, &serial);
        }
        libobsbot_core::Event::DeviceRemoved { serial } => {
            (*out_event).kind = OBSBOT_EVENT_DEVICE_REMOVED;
            copy_into_buf((*out_event).serial.as_mut_ptr(), OBSBOT_STR_MAX, &serial);
        }
        libobsbot_core::Event::Status { serial, snapshot } => {
            (*out_event).kind = OBSBOT_EVENT_STATUS;
            copy_into_buf((*out_event).serial.as_mut_ptr(), OBSBOT_STR_MAX, &serial);
            copy_into_buf(
                (*out_event).status.firmware.as_mut_ptr(),
                OBSBOT_STR_MAX,
                &snapshot.firmware,
            );
            copy_into_buf(
                (*out_event).status.serial.as_mut_ptr(),
                OBSBOT_STR_MAX,
                &snapshot.serial,
            );
            (*out_event).status.brightness = snapshot.brightness;
            (*out_event).status.contrast = snapshot.contrast;
            (*out_event).status.saturation = snapshot.saturation;
            (*out_event).status.zoom = snapshot.zoom;
            (*out_event).status.pan = snapshot.pan;
            (*out_event).status.tilt = snapshot.tilt;
        }
        _ => {
            (*out_event).kind = 0;
        }
    }
    OBSBOT_OK
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

/// Set image hue (PU). i16 LE on the wire; out-of-range values
/// return `OBSBOT_ERR_OUT_OF_RANGE`.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_hue(handle: *mut ObsbotDevice, value: i32) -> c_int {
    with_device(handle, |d| d.set_hue(value))
}

/// Read current image hue into `*out_value`.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_hue(
    handle: *mut ObsbotDevice,
    out_value: *mut i32,
) -> c_int {
    read_into(handle, out_value, Device::hue)
}

/// Set image sharpness (PU). u16 LE.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_sharpness(
    handle: *mut ObsbotDevice,
    value: i32,
) -> c_int {
    with_device(handle, |d| d.set_sharpness(value))
}

/// Read current image sharpness into `*out_value`.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_sharpness(
    handle: *mut ObsbotDevice,
    out_value: *mut i32,
) -> c_int {
    read_into(handle, out_value, Device::sharpness)
}

/// Set sensor gain (PU). u16 LE.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_gain(handle: *mut ObsbotDevice, value: i32) -> c_int {
    with_device(handle, |d| d.set_gain(value))
}

/// Read current sensor gain into `*out_value`.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_gain(
    handle: *mut ObsbotDevice,
    out_value: *mut i32,
) -> c_int {
    read_into(handle, out_value, Device::gain)
}

/// Set backlight compensation (PU). u16 LE; 0 disables it.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_backlight_compensation(
    handle: *mut ObsbotDevice,
    value: i32,
) -> c_int {
    with_device(handle, |d| d.set_backlight_compensation(value))
}

/// Read current backlight-compensation value into `*out_value`.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_backlight_compensation(
    handle: *mut ObsbotDevice,
    out_value: *mut i32,
) -> c_int {
    read_into(handle, out_value, Device::backlight_compensation)
}

/// Set anti-flicker mode (PU): 0 = Off, 1 = 50 Hz, 2 = 60 Hz, 3 = Auto.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_anti_flicker(
    handle: *mut ObsbotDevice,
    mode: c_int,
) -> c_int {
    let Some(m) = anti_flicker_from_int(mode) else {
        return OBSBOT_ERR_OUT_OF_RANGE;
    };
    with_device(handle, |d| d.set_anti_flicker(m))
}

/// Read current anti-flicker mode into `*out_mode` (same encoding).
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_anti_flicker(
    handle: *mut ObsbotDevice,
    out_mode: *mut c_int,
) -> c_int {
    if handle.is_null() || out_mode.is_null() {
        return OBSBOT_ERR_NOT_FOUND;
    }
    match (*handle).0.anti_flicker() {
        Ok(m) => {
            *out_mode = anti_flicker_to_int(m);
            OBSBOT_OK
        }
        Err(e) => map_error(&e),
    }
}

/// Enable or disable autofocus (CT).
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_auto_focus(
    handle: *mut ObsbotDevice,
    on: c_int,
) -> c_int {
    with_device(handle, |d| d.set_auto_focus(on != 0))
}

/// Whether autofocus is currently enabled.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_auto_focus(
    handle: *mut ObsbotDevice,
    out_on: *mut c_int,
) -> c_int {
    if handle.is_null() || out_on.is_null() {
        return OBSBOT_ERR_NOT_FOUND;
    }
    match (*handle).0.auto_focus() {
        Ok(b) => {
            *out_on = c_int::from(b);
            OBSBOT_OK
        }
        Err(e) => map_error(&e),
    }
}

/// Set auto-exposure mode: 0 = Manual, 1 = Auto, 2 = `ShutterPriority`,
/// 3 = `AperturePriority`.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_ae_mode(
    handle: *mut ObsbotDevice,
    mode: c_int,
) -> c_int {
    let Some(m) = ae_mode_from_int(mode) else {
        return OBSBOT_ERR_OUT_OF_RANGE;
    };
    with_device(handle, |d| d.set_ae_mode(m))
}

/// Read current auto-exposure mode into `*out_mode` (same encoding).
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_ae_mode(
    handle: *mut ObsbotDevice,
    out_mode: *mut c_int,
) -> c_int {
    if handle.is_null() || out_mode.is_null() {
        return OBSBOT_ERR_NOT_FOUND;
    }
    match (*handle).0.ae_mode() {
        Ok(m) => {
            *out_mode = ae_mode_to_int(m);
            OBSBOT_OK
        }
        Err(e) => map_error(&e),
    }
}

/// Lock or unlock auto-exposure. Convenience over
/// [`obsbot_device_set_ae_mode`].
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_ae_lock(
    handle: *mut ObsbotDevice,
    locked: c_int,
) -> c_int {
    with_device(handle, |d| d.set_ae_lock(locked != 0))
}

/// Set manual exposure time in 100 us units (CT).
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_exposure_time(
    handle: *mut ObsbotDevice,
    value_100us: u32,
) -> c_int {
    with_device(handle, |d| d.set_exposure_time(value_100us))
}

/// Read current exposure time (100 us units) into `*out_value`.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_exposure_time(
    handle: *mut ObsbotDevice,
    out_value: *mut u32,
) -> c_int {
    read_into(handle, out_value, Device::exposure_time)
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

/// Read current HDR mode into `*out_mode` (same encoding as
/// [`obsbot_device_set_wdr`]).
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_wdr(
    handle: *mut ObsbotDevice,
    out_mode: *mut c_int,
) -> c_int {
    if handle.is_null() || out_mode.is_null() {
        return OBSBOT_ERR_NOT_FOUND;
    }
    match (*handle).0.wdr() {
        Ok(libobsbot_core::WdrMode::Off) => {
            *out_mode = 0;
            OBSBOT_OK
        }
        Ok(libobsbot_core::WdrMode::Dol2To1) => {
            *out_mode = 1;
            OBSBOT_OK
        }
        Err(e) => map_error(&e),
    }
}

/// Read current face-AE state into `*out_on` (`0` = off, `1` = on).
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_face_ae(
    handle: *mut ObsbotDevice,
    out_on: *mut c_int,
) -> c_int {
    if handle.is_null() || out_on.is_null() {
        return OBSBOT_ERR_NOT_FOUND;
    }
    match (*handle).0.face_ae() {
        Ok(b) => {
            *out_on = c_int::from(b);
            OBSBOT_OK
        }
        Err(e) => map_error(&e),
    }
}

/// Read current AI master mode into `*out_mode` (same encoding as
/// [`obsbot_device_set_ai_mode`]).
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_ai_mode(
    handle: *mut ObsbotDevice,
    out_mode: *mut c_int,
) -> c_int {
    if handle.is_null() || out_mode.is_null() {
        return OBSBOT_ERR_NOT_FOUND;
    }
    match (*handle).0.ai_mode() {
        Ok(m) => {
            *out_mode = match m {
                AiMode::None => 0,
                AiMode::Group => 1,
                AiMode::Human => 2,
                AiMode::Hand => 3,
                AiMode::WhiteBoard => 4,
                AiMode::Desk => 5,
            };
            OBSBOT_OK
        }
        Err(e) => map_error(&e),
    }
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

/// Microphone Automatic Gain Control: non-zero on, zero off.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_set_audio_agc(
    handle: *mut ObsbotDevice,
    on: c_int,
) -> c_int {
    with_device(handle, |d| d.set_audio_agc(on != 0))
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

/// Read current white-balance mode + Kelvin value. `mode` follows the
/// same encoding as [`obsbot_device_set_white_balance`].
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_white_balance(
    handle: *mut ObsbotDevice,
    out_mode: *mut c_int,
    out_kelvin: *mut u16,
) -> c_int {
    if handle.is_null() || out_mode.is_null() || out_kelvin.is_null() {
        return OBSBOT_ERR_NOT_FOUND;
    }
    match (*handle).0.white_balance() {
        Ok((m, k)) => {
            *out_mode = match m {
                WhiteBalanceMode::Auto => 0,
                WhiteBalanceMode::Manual => 1,
            };
            *out_kelvin = k;
            OBSBOT_OK
        }
        Err(e) => map_error(&e),
    }
}

/// Read current pan + tilt as normalised values in -1.0..=1.0.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_pan_tilt(
    handle: *mut ObsbotDevice,
    out_pan: *mut f32,
    out_tilt: *mut f32,
) -> c_int {
    if handle.is_null() || out_pan.is_null() || out_tilt.is_null() {
        return OBSBOT_ERR_NOT_FOUND;
    }
    match (*handle).0.pan_tilt() {
        Ok((p, t)) => {
            *out_pan = p;
            *out_tilt = t;
            OBSBOT_OK
        }
        Err(e) => map_error(&e),
    }
}

/// Read current zoom value.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_zoom(handle: *mut ObsbotDevice, out: *mut f32) -> c_int {
    read_into(handle, out, Device::zoom)
}

/// Read current focus value.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_focus(handle: *mut ObsbotDevice, out: *mut f32) -> c_int {
    read_into(handle, out, Device::focus)
}

/// Plain-old-data form of [`libobsbot_core::Status`] for C consumers.
/// `firmware` and `serial` are NUL-terminated; strings longer than
/// `OBSBOT_STR_MAX - 1` bytes are truncated. The unused tail is
/// zero-padded.
#[repr(C)]
pub struct ObsbotStatus {
    /// Camera-reported firmware version, e.g. "4.4.6.1". NUL-terminated.
    pub firmware: [c_char; OBSBOT_STR_MAX],
    /// Camera-reported serial number. NUL-terminated.
    pub serial: [c_char; OBSBOT_STR_MAX],
    /// Brightness reported by the Processing Unit.
    pub brightness: i32,
    /// Contrast reported by the Processing Unit.
    pub contrast: i32,
    /// Saturation reported by the Processing Unit.
    pub saturation: i32,
    /// Current zoom value (raw u16 cast to f32 for now).
    pub zoom: f32,
    /// Normalised pan in -1.0..=1.0.
    pub pan: f32,
    /// Normalised tilt in -1.0..=1.0.
    pub tilt: f32,
}

/// Maximum string length (including NUL) for [`ObsbotStatus`] fields.
pub const OBSBOT_STR_MAX: usize = 64;

/// Read a synchronous status snapshot into `*out`. Returns
/// `OBSBOT_ERR_NOT_FOUND` for NULL handles. Best-effort: individual
/// read failures leave the corresponding field at its default value
/// rather than failing the whole call.
#[no_mangle]
pub unsafe extern "C" fn obsbot_device_status(
    handle: *mut ObsbotDevice,
    out: *mut ObsbotStatus,
) -> c_int {
    if handle.is_null() || out.is_null() {
        return OBSBOT_ERR_NOT_FOUND;
    }
    let snap = match (*handle).0.status() {
        Ok(s) => s,
        Err(e) => return map_error(&e),
    };
    ptr::write_bytes(out, 0, 1);
    copy_into_buf((*out).firmware.as_mut_ptr(), OBSBOT_STR_MAX, &snap.firmware);
    copy_into_buf((*out).serial.as_mut_ptr(), OBSBOT_STR_MAX, &snap.serial);
    (*out).brightness = snap.brightness;
    (*out).contrast = snap.contrast;
    (*out).saturation = snap.saturation;
    (*out).zoom = snap.zoom;
    (*out).pan = snap.pan;
    (*out).tilt = snap.tilt;
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

/// Truncating copy of `s` into a fixed C buffer; the buffer is
/// always NUL-terminated even if `s` is longer than `cap - 1`.
unsafe fn copy_into_buf(buf: *mut c_char, cap: usize, s: &str) {
    if cap == 0 {
        return;
    }
    let n = s.len().min(cap - 1);
    ptr::copy_nonoverlapping(s.as_ptr().cast::<c_char>(), buf, n);
    *buf.add(n) = 0;
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

const fn anti_flicker_from_int(v: c_int) -> Option<AntiFlicker> {
    match v {
        0 => Some(AntiFlicker::Off),
        1 => Some(AntiFlicker::Hz50),
        2 => Some(AntiFlicker::Hz60),
        3 => Some(AntiFlicker::Auto),
        _ => None,
    }
}

const fn anti_flicker_to_int(m: AntiFlicker) -> c_int {
    match m {
        AntiFlicker::Off => 0,
        AntiFlicker::Hz50 => 1,
        AntiFlicker::Hz60 => 2,
        AntiFlicker::Auto => 3,
    }
}

const fn ae_mode_from_int(v: c_int) -> Option<AeMode> {
    match v {
        0 => Some(AeMode::Manual),
        1 => Some(AeMode::Auto),
        2 => Some(AeMode::ShutterPriority),
        3 => Some(AeMode::AperturePriority),
        _ => None,
    }
}

const fn ae_mode_to_int(m: AeMode) -> c_int {
    match m {
        AeMode::Manual => 0,
        AeMode::Auto => 1,
        AeMode::ShutterPriority => 2,
        AeMode::AperturePriority => 3,
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
