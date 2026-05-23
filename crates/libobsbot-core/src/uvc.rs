// SPDX-License-Identifier: GPL-3.0-only
//! Standard UVC entity ids and control selectors.
//!
//! Source: USB Device Class Definition for Video Devices, Revision 1.5,
//! sections A.9.4 (`CameraTerminal`) and A.9.5 (`ProcessingUnit`). These
//! constants are normative - no audit-trail capture required.

/// Camera Terminal entity id on the OBSBOT Meet 2 (matches descriptor).
pub(crate) const CAMERA_TERMINAL: u8 = 1;

/// Processing Unit entity id on the OBSBOT Meet 2 (matches descriptor).
pub(crate) const PROCESSING_UNIT: u8 = 3;

/// `CameraTerminal` control selectors (UVC 1.5 §A.9.4).
pub(crate) mod ct {
    /// `CT_AE_MODE_CONTROL`; u8 bitmap (1 = Manual, 2 = Auto,
    /// 4 = Shutter Priority, 8 = Aperture Priority).
    pub(crate) const AE_MODE: u8 = 0x02;
    /// `CT_EXPOSURE_TIME_ABSOLUTE_CONTROL`; u32 LE in 100 us units, 4 bytes.
    pub(crate) const EXPOSURE_TIME_ABSOLUTE: u8 = 0x04;
    /// `CT_FOCUS_ABSOLUTE_CONTROL`; u16 LE focal-length-like value, 2 bytes.
    pub(crate) const FOCUS_ABSOLUTE: u8 = 0x06;
    /// `CT_FOCUS_AUTO_CONTROL`; bool, 1 byte.
    pub(crate) const FOCUS_AUTO: u8 = 0x08;
    /// `CT_ZOOM_ABSOLUTE_CONTROL`; u16 LE objective focal length, 2 bytes.
    pub(crate) const ZOOM_ABSOLUTE: u8 = 0x0b;
    /// `CT_PANTILT_ABSOLUTE_CONTROL`; i32 LE pan + i32 LE tilt arc-seconds, 8 bytes.
    pub(crate) const PANTILT_ABSOLUTE: u8 = 0x0d;
}

/// `ProcessingUnit` control selectors (UVC 1.5 §A.9.5).
pub(crate) mod pu {
    /// `PU_BRIGHTNESS_CONTROL`; i16 LE, 2 bytes.
    pub(crate) const BRIGHTNESS: u8 = 0x02;
    /// `PU_CONTRAST_CONTROL`; u16 LE, 2 bytes.
    pub(crate) const CONTRAST: u8 = 0x03;
    /// `PU_HUE_CONTROL`; i16 LE, 2 bytes.
    pub(crate) const HUE: u8 = 0x06;
    /// `PU_SATURATION_CONTROL`; u16 LE, 2 bytes.
    pub(crate) const SATURATION: u8 = 0x07;
    /// `PU_SHARPNESS_CONTROL`; u16 LE, 2 bytes.
    pub(crate) const SHARPNESS: u8 = 0x08;
    /// `PU_BACKLIGHT_COMPENSATION_CONTROL`; u16 LE, 2 bytes.
    pub(crate) const BACKLIGHT_COMPENSATION: u8 = 0x09;
    /// `PU_GAIN_CONTROL`; u16 LE, 2 bytes.
    pub(crate) const GAIN: u8 = 0x04;
    /// `PU_POWER_LINE_FREQUENCY_CONTROL`; u8, 1 byte
    /// (0 = Disabled, 1 = 50 Hz, 2 = 60 Hz, 3 = Auto).
    pub(crate) const POWER_LINE_FREQUENCY: u8 = 0x05;
    /// `PU_WHITE_BALANCE_TEMPERATURE_CONTROL`; u16 LE Kelvin, 2 bytes.
    pub(crate) const WHITE_BALANCE_TEMPERATURE: u8 = 0x0a;
    /// `PU_WHITE_BALANCE_TEMPERATURE_AUTO_CONTROL`; bool, 1 byte.
    pub(crate) const WHITE_BALANCE_TEMPERATURE_AUTO: u8 = 0x0b;
}

/// Class-specific `GET` request variant for the `Transport::uvc_get` boundary.
///
/// Values match the UVC `bRequest` byte one-to-one (UVC 1.5 §4.2.1). Only
/// the variants currently consumed by [`crate::Device`] methods are listed;
/// the remaining UVC `GET_*` requests (`RES`, `LEN`, `INFO`, `DEF`) are
/// added as Device methods grow to need them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UvcGet {
    /// `GET_CUR`. Current value.
    Cur = 0x81,
    /// `GET_MIN`. Lower bound of the supported range.
    Min = 0x82,
    /// `GET_MAX`. Upper bound of the supported range.
    Max = 0x83,
}
