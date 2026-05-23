// SPDX-License-Identifier: GPL-3.0-only
//! Public enums and value types exposed by the SDK.

/// OBSBOT camera model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ProductType {
    /// OBSBOT Meet 2 - the only model supported in v1.
    Meet2,
}

/// Field-of-view preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FovType {
    /// ~86° - widest.
    Wide,
    /// ~78° - medium.
    Medium,
    /// ~65° - narrowest.
    Narrow,
}

/// Auto-exposure mode. Maps onto the UVC `CT_AE_MODE_CONTROL`
/// bitmap from §4.2.2.1.2 (only one bit is set at a time on a
/// SET; the GET reply may have multiple bits indicating supported
/// modes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AeMode {
    /// Manual exposure, manual gain.
    Manual,
    /// Automatic exposure, automatic gain. The default.
    Auto,
    /// Manual exposure, automatic gain.
    ShutterPriority,
    /// Automatic exposure, manual gain.
    AperturePriority,
}

/// Mains frequency for the anti-flicker algorithm. Maps onto the UVC
/// `PU_POWER_LINE_FREQUENCY_CONTROL` value byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AntiFlicker {
    /// No flicker compensation.
    Off,
    /// 50 Hz mains (Europe, most of Asia, Australia).
    Hz50,
    /// 60 Hz mains (North America, parts of Asia).
    Hz60,
    /// Camera picks based on ambient light analysis.
    Auto,
}

/// HDR / wide-dynamic-range mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WdrMode {
    /// HDR off.
    Off,
    /// Digital overlap, 2-to-1.
    Dol2To1,
}

/// White-balance preset. The SDK's `cameraSetWhiteBalanceR`
/// enumerates many presets, but only `Auto` and `Manual` are
/// supported on the Meet 2 (per the SDK header). The other
/// presets (Daylight, Fluorescent, etc.) are Tiny / Tail-Air
/// only and intentionally absent here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhiteBalanceMode {
    /// Automatic white balance.
    Auto,
    /// Manual Kelvin temperature; the value lives in [`crate::Device::white_balance`].
    Manual,
}

/// Auto-framing sub-mode for the Meet 2.
/// Maps onto the SDK's `cameraSetAutoFramingModeU(group_single,
/// close_upper)` pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoFramingMode {
    /// Multi-person framing.
    Group,
    /// Single subject, close-up framing.
    SingleCloseUp,
    /// Single subject, upper-body framing.
    SingleUpperBody,
}

/// AI master mode. Matches `Device::AiWorkModeType` in the OBSBOT
/// public SDK for the Tiny / Tiny SE / Meet 2 family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiMode {
    /// AI features off.
    None,
    /// Multi-person tracking.
    Group,
    /// Single-person tracking.
    Human,
    /// Hand-gesture tracking.
    Hand,
    /// Whiteboard mode.
    WhiteBoard,
    /// Desk mode.
    Desk,
}

/// Media mode. Matches `Device::MediaMode` in the OBSBOT public SDK
/// (`MediaModeNormal = 0`, `MediaModeBackground = 1`, `MediaModeAutoFrame = 2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaMode {
    /// Normal UVC.
    Normal,
    /// Virtual-background mode.
    Background,
    /// Camera-side auto-framing.
    AutoFrame,
}

/// How often the per-device status poller should sample state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cadence {
    /// Slow polling for idle UIs (every 2.5 s).
    Slow,
    /// Fast polling for live UIs that need responsive readouts (every 25 ms).
    Fast,
}

impl Cadence {
    /// Sampling period in milliseconds.
    #[must_use]
    pub const fn period_ms(self) -> u32 {
        match self {
            Cadence::Slow => 2500,
            Cadence::Fast => 25,
        }
    }
}

/// Snapshot of camera state pushed periodically by the status poller.
#[derive(Debug, Clone, Default)]
pub struct Status {
    /// Camera-reported firmware version, e.g. `"4.4.6.1"`. Empty when
    /// the firmware read failed.
    pub firmware: String,
    /// Camera-reported serial number. Empty when the serial read failed.
    pub serial: String,
    /// Brightness setting reported by the camera.
    pub brightness: i32,
    /// Contrast setting reported by the camera.
    pub contrast: i32,
    /// Saturation setting reported by the camera.
    pub saturation: i32,
    /// Current zoom value.
    pub zoom: f32,
    /// Pan (-1.0 ..= 1.0).
    pub pan: f32,
    /// Tilt (-1.0 ..= 1.0).
    pub tilt: f32,
}
