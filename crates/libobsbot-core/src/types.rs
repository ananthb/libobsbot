// SPDX-License-Identifier: GPL-3.0-only
//! Public enums and value types exposed by the SDK.

/// OBSBOT camera model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ProductType {
    /// OBSBOT Meet 2 — the only model supported in v1.
    Meet2,
}

/// Field-of-view preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FovType {
    /// ~86° — widest.
    Wide,
    /// ~78° — medium.
    Medium,
    /// ~65° — narrowest.
    Narrow,
}

/// HDR / wide-dynamic-range mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WdrMode {
    /// HDR off.
    Off,
    /// Digital overlap, 2-to-1.
    Dol2To1,
}

/// White-balance preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhiteBalanceMode {
    /// Automatic white balance.
    Auto,
    /// Manual Kelvin temperature; the value lives in [`crate::Device::white_balance`].
    Manual,
    /// Daylight preset.
    Daylight,
    /// Fluorescent preset.
    Fluorescent,
    /// Tungsten preset.
    Tungsten,
}

/// AI auto-framing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoFramingMode {
    /// Single subject, head-and-shoulders.
    SingleHeadShoulders,
    /// Single subject, upper body.
    SingleUpperBody,
    /// Group framing.
    Group,
}

/// AI master mode (off / auto-framing / hand tracking, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiMode {
    /// AI features off.
    Off,
    /// AI face/body tracking on.
    On,
}

/// Media mode (normal capture vs streaming).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaMode {
    /// Normal UVC.
    Normal,
    /// Auto-framing on the camera-side.
    AutoFraming,
    /// Streaming mode.
    Streaming,
}

/// AI tracking speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackSpeed {
    /// Slow / smooth tracking.
    Slow,
    /// Normal tracking speed.
    Normal,
    /// Fast / snappy tracking.
    Fast,
}

/// Snapshot of camera state pushed periodically by the status poller.
#[derive(Debug, Clone, Default)]
pub struct Status {
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
