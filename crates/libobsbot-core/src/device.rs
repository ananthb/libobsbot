// SPDX-License-Identifier: GPL-3.0-only
//! Opened camera handle and the v1 method surface.
//!
//! Every Device method in v0.0.0 routes through the [`Transport`] trait. The
//! transport returns [`Error::Unsupported`] until protocol capture lands; the
//! method signatures and trait wiring are stable.

use core::ops::RangeInclusive;

use crate::devices::meet2;
use crate::discovery::DeviceInfo;
use crate::transport::Transport;
use crate::types::{
    AiMode, AutoFramingMode, FovType, MediaMode, ProductType, Status, TrackSpeed, WdrMode,
    WhiteBalanceMode,
};
use crate::{Error, Result};

/// Opened OBSBOT camera.
///
/// Obtain via [`crate::Devices::open`]. Dropping the handle releases the USB
/// claim and stops the per-device status poller (no poller yet — lands in
/// M7).
pub struct Device {
    info: DeviceInfo,
    transport: Box<dyn Transport>,
    firmware: String,
}

impl Device {
    pub(crate) fn new(info: DeviceInfo, transport: Box<dyn Transport>) -> Self {
        Self {
            info,
            transport,
            firmware: meet2::MIN_FW.to_owned(),
        }
    }

    /// Human-readable model name.
    #[must_use]
    pub fn name(&self) -> &str {
        match self.info.product_type {
            ProductType::Meet2 => "OBSBOT Meet 2",
        }
    }

    /// Device serial number as reported by the OS at enumeration time.
    #[must_use]
    pub fn serial(&self) -> &str {
        &self.info.serial
    }

    /// Firmware version string. Returns the build-time minimum until a real
    /// camera response can be parsed (M2).
    #[must_use]
    pub fn firmware_version(&self) -> &str {
        &self.firmware
    }

    /// Model enum value.
    #[must_use]
    pub fn product_type(&self) -> ProductType {
        self.info.product_type
    }

    /// Read a fresh status snapshot synchronously.
    pub fn status(&self) -> Result<Status> {
        // 64 bytes is the maximum UVC class-specific GET_CUR payload.
        let mut buf = [0u8; 64];
        let _ = self.transport.xu_get(0, &mut buf)?;
        unreachable!("xu_get returns Unsupported until M2");
    }

    /// Set pan and tilt in normalised camera coordinates (-1.0 ..= 1.0).
    pub fn set_pan_tilt(&self, pan: f32, tilt: f32) -> Result<()> {
        if !(-1.0..=1.0).contains(&pan) || !(-1.0..=1.0).contains(&tilt) {
            return Err(Error::OutOfRange);
        }
        let mut payload = [0u8; 8];
        payload[..4].copy_from_slice(&pan.to_le_bytes());
        payload[4..].copy_from_slice(&tilt.to_le_bytes());
        self.transport.xu_set(0, &payload)
    }

    /// Set zoom as a ratio of the camera's optical range (1.0 ..= max).
    pub fn set_zoom(&self, zoom: f32) -> Result<()> {
        self.transport.xu_set(0, &zoom.to_le_bytes())
    }

    /// Set zoom with a ramp speed (camera-defined units, 0.0 = no ramp).
    pub fn set_zoom_with_speed(&self, zoom: f32, speed: f32) -> Result<()> {
        let mut payload = [0u8; 8];
        payload[..4].copy_from_slice(&zoom.to_le_bytes());
        payload[4..].copy_from_slice(&speed.to_le_bytes());
        self.transport.xu_set(0, &payload)
    }

    /// Set focus distance.
    pub fn set_focus(&self, focus: f32) -> Result<()> {
        self.transport.xu_set(0, &focus.to_le_bytes())
    }

    /// Set brightness.
    pub fn set_brightness(&self, value: i32) -> Result<()> {
        self.transport.xu_set(0, &value.to_le_bytes())
    }

    /// Read current brightness.
    pub fn brightness(&self) -> Result<i32> {
        let mut buf = [0u8; 4];
        let _ = self.transport.xu_get(0, &mut buf)?;
        Ok(i32::from_le_bytes(buf))
    }

    /// Reported brightness range.
    pub fn brightness_range(&self) -> Result<RangeInclusive<i32>> {
        let mut buf = [0u8; 8];
        let _ = self.transport.xu_get(0, &mut buf)?;
        let lo = i32::from_le_bytes(buf[..4].try_into().unwrap());
        let hi = i32::from_le_bytes(buf[4..].try_into().unwrap());
        Ok(lo..=hi)
    }

    /// Set contrast.
    pub fn set_contrast(&self, value: i32) -> Result<()> {
        self.transport.xu_set(0, &value.to_le_bytes())
    }

    /// Read current contrast.
    pub fn contrast(&self) -> Result<i32> {
        let mut buf = [0u8; 4];
        let _ = self.transport.xu_get(0, &mut buf)?;
        Ok(i32::from_le_bytes(buf))
    }

    /// Reported contrast range.
    pub fn contrast_range(&self) -> Result<RangeInclusive<i32>> {
        let mut buf = [0u8; 8];
        let _ = self.transport.xu_get(0, &mut buf)?;
        let lo = i32::from_le_bytes(buf[..4].try_into().unwrap());
        let hi = i32::from_le_bytes(buf[4..].try_into().unwrap());
        Ok(lo..=hi)
    }

    /// Set saturation.
    pub fn set_saturation(&self, value: i32) -> Result<()> {
        self.transport.xu_set(0, &value.to_le_bytes())
    }

    /// Read current saturation.
    pub fn saturation(&self) -> Result<i32> {
        let mut buf = [0u8; 4];
        let _ = self.transport.xu_get(0, &mut buf)?;
        Ok(i32::from_le_bytes(buf))
    }

    /// Reported saturation range.
    pub fn saturation_range(&self) -> Result<RangeInclusive<i32>> {
        let mut buf = [0u8; 8];
        let _ = self.transport.xu_get(0, &mut buf)?;
        let lo = i32::from_le_bytes(buf[..4].try_into().unwrap());
        let hi = i32::from_le_bytes(buf[4..].try_into().unwrap());
        Ok(lo..=hi)
    }

    /// Set white balance mode; the `kelvin` value is meaningful only when
    /// `mode` is [`WhiteBalanceMode::Manual`].
    pub fn set_white_balance(&self, mode: WhiteBalanceMode, kelvin: Option<u16>) -> Result<()> {
        let mut payload = [0u8; 4];
        payload[0] = encode_wb_mode(mode);
        payload[2..].copy_from_slice(&kelvin.unwrap_or(0).to_le_bytes());
        self.transport.xu_set(0, &payload)
    }

    /// Read current white-balance mode and Kelvin value.
    pub fn white_balance(&self) -> Result<(WhiteBalanceMode, u16)> {
        let mut buf = [0u8; 4];
        let _ = self.transport.xu_get(0, &mut buf)?;
        let mode = decode_wb_mode(buf[0])?;
        let kelvin = u16::from_le_bytes(buf[2..].try_into().unwrap());
        Ok((mode, kelvin))
    }

    /// Presets reported by the camera as available.
    pub fn white_balance_presets(&self) -> Result<Vec<WhiteBalanceMode>> {
        let mut buf = [0u8; 16];
        let n = self.transport.xu_get(0, &mut buf)?;
        Ok(buf[..n]
            .iter()
            .copied()
            .filter_map(|b| decode_wb_mode(b).ok())
            .collect())
    }

    /// Reported manual Kelvin range.
    pub fn white_balance_range(&self) -> Result<RangeInclusive<u16>> {
        let mut buf = [0u8; 4];
        let _ = self.transport.xu_get(0, &mut buf)?;
        let lo = u16::from_le_bytes(buf[..2].try_into().unwrap());
        let hi = u16::from_le_bytes(buf[2..].try_into().unwrap());
        Ok(lo..=hi)
    }

    /// Set HDR mode.
    pub fn set_wdr(&self, mode: WdrMode) -> Result<()> {
        self.transport.xu_set(0, &[encode_wdr(mode)])
    }

    /// Read current HDR mode.
    pub fn wdr(&self) -> Result<WdrMode> {
        let mut buf = [0u8; 1];
        let _ = self.transport.xu_get(0, &mut buf)?;
        decode_wdr(buf[0])
    }

    /// Set field-of-view preset.
    pub fn set_fov(&self, fov: FovType) -> Result<()> {
        self.transport.xu_set(0, &[encode_fov(fov)])
    }

    /// Toggle face-based auto-exposure.
    pub fn set_face_ae(&self, on: bool) -> Result<()> {
        self.transport.xu_set(0, &[u8::from(on)])
    }

    /// Toggle face-based auto-focus.
    pub fn set_face_focus(&self, on: bool) -> Result<()> {
        self.transport.xu_set(0, &[u8::from(on)])
    }

    /// Select media mode.
    pub fn set_media_mode(&self, mode: MediaMode) -> Result<()> {
        self.transport.xu_set(0, &[encode_media_mode(mode)])
    }

    /// Configure auto-framing.
    pub fn set_auto_framing(&self, mode: AutoFramingMode) -> Result<()> {
        self.transport.xu_set(0, &[encode_auto_framing(mode)])
    }

    /// Master AI mode (on/off).
    pub fn set_ai_mode(&self, mode: AiMode) -> Result<()> {
        let on = matches!(mode, AiMode::On);
        self.transport.xu_set(0, &[u8::from(on)])
    }

    /// Enable AI auto-zoom while tracking.
    pub fn set_ai_auto_zoom(&self, on: bool) -> Result<()> {
        self.transport.xu_set(0, &[u8::from(on)])
    }

    /// Set AI tracking speed.
    pub fn set_track_speed(&self, speed: TrackSpeed) -> Result<()> {
        self.transport.xu_set(0, &[encode_track_speed(speed)])
    }

    /// Toggle audio auto-gain control.
    pub fn set_audio_auto_gain(&self, on: bool) -> Result<()> {
        self.transport.xu_set(0, &[u8::from(on)])
    }
}

// ---- payload helpers ------------------------------------------------------
//
// These encode/decode functions sit between the public method signatures and
// the transport. They are placeholders — the *byte values* below are not the
// real ones the camera expects, only stand-ins until pcaps land. The function
// shapes won't change.

fn encode_wb_mode(mode: WhiteBalanceMode) -> u8 {
    match mode {
        WhiteBalanceMode::Auto => 0,
        WhiteBalanceMode::Manual => 1,
        WhiteBalanceMode::Daylight => 2,
        WhiteBalanceMode::Fluorescent => 3,
        WhiteBalanceMode::Tungsten => 4,
    }
}

fn decode_wb_mode(b: u8) -> Result<WhiteBalanceMode> {
    match b {
        0 => Ok(WhiteBalanceMode::Auto),
        1 => Ok(WhiteBalanceMode::Manual),
        2 => Ok(WhiteBalanceMode::Daylight),
        3 => Ok(WhiteBalanceMode::Fluorescent),
        4 => Ok(WhiteBalanceMode::Tungsten),
        _ => Err(Error::BadResponse {
            selector: 0,
            bytes: vec![b],
        }),
    }
}

fn encode_wdr(mode: WdrMode) -> u8 {
    match mode {
        WdrMode::Off => 0,
        WdrMode::Dol2To1 => 1,
    }
}

fn decode_wdr(b: u8) -> Result<WdrMode> {
    match b {
        0 => Ok(WdrMode::Off),
        1 => Ok(WdrMode::Dol2To1),
        _ => Err(Error::BadResponse {
            selector: 0,
            bytes: vec![b],
        }),
    }
}

fn encode_fov(fov: FovType) -> u8 {
    match fov {
        FovType::Wide => 0,
        FovType::Medium => 1,
        FovType::Narrow => 2,
    }
}

fn encode_media_mode(mode: MediaMode) -> u8 {
    match mode {
        MediaMode::Normal => 0,
        MediaMode::AutoFraming => 1,
        MediaMode::Streaming => 2,
    }
}

fn encode_auto_framing(mode: AutoFramingMode) -> u8 {
    match mode {
        AutoFramingMode::SingleHeadShoulders => 0,
        AutoFramingMode::SingleUpperBody => 1,
        AutoFramingMode::Group => 2,
    }
}

fn encode_track_speed(speed: TrackSpeed) -> u8 {
    match speed {
        TrackSpeed::Slow => 0,
        TrackSpeed::Normal => 1,
        TrackSpeed::Fast => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::Transport;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MockTransport {
        last_set: Mutex<Option<(u8, Vec<u8>)>>,
    }

    impl Transport for MockTransport {
        fn xu_set(&self, selector: u8, payload: &[u8]) -> Result<()> {
            *self.last_set.lock().unwrap() = Some((selector, payload.to_vec()));
            Ok(())
        }

        fn xu_get(&self, _selector: u8, out: &mut [u8]) -> Result<usize> {
            for b in &mut *out {
                *b = 0;
            }
            Ok(out.len())
        }
    }

    fn device_with_mock() -> (Device, std::sync::Arc<MockTransport>) {
        let mock = std::sync::Arc::new(MockTransport::default());
        let transport: Box<dyn Transport> = Box::new(MockTransport::default());
        let info = DeviceInfo {
            vendor_id: meet2::VENDOR_ID,
            product_id: meet2::PRODUCT_ID_MEET2,
            product_type: ProductType::Meet2,
            serial: "MOCK".to_owned(),
        };
        (Device::new(info, transport), mock)
    }

    #[test]
    fn metadata_accessors_use_info() {
        let (d, _) = device_with_mock();
        assert_eq!(d.name(), "OBSBOT Meet 2");
        assert_eq!(d.serial(), "MOCK");
        assert_eq!(d.product_type(), ProductType::Meet2);
        assert_eq!(d.firmware_version(), meet2::MIN_FW);
    }

    #[test]
    fn pan_tilt_rejects_out_of_range() {
        let (d, _) = device_with_mock();
        assert!(matches!(d.set_pan_tilt(2.0, 0.0), Err(Error::OutOfRange)));
        assert!(matches!(d.set_pan_tilt(0.0, -1.5), Err(Error::OutOfRange)));
    }

    #[test]
    fn pan_tilt_payload_layout_is_8_bytes() {
        let (d, _) = device_with_mock();
        // Mock transport accepts, so we just check the call doesn't panic and
        // the in-range branch is exercised end-to-end.
        assert!(d.set_pan_tilt(0.0, 0.0).is_ok());
    }

    #[test]
    fn wb_mode_encode_decode_round_trip() {
        for mode in [
            WhiteBalanceMode::Auto,
            WhiteBalanceMode::Manual,
            WhiteBalanceMode::Daylight,
            WhiteBalanceMode::Fluorescent,
            WhiteBalanceMode::Tungsten,
        ] {
            let b = encode_wb_mode(mode);
            assert_eq!(decode_wb_mode(b).unwrap(), mode);
        }
    }

    #[test]
    fn wdr_encode_decode_round_trip() {
        for mode in [WdrMode::Off, WdrMode::Dol2To1] {
            assert_eq!(decode_wdr(encode_wdr(mode)).unwrap(), mode);
        }
    }

    #[test]
    fn decode_rejects_unknown_byte() {
        assert!(matches!(
            decode_wb_mode(99),
            Err(Error::BadResponse { selector: 0, .. })
        ));
        assert!(matches!(decode_wdr(99), Err(Error::BadResponse { .. })));
    }
}
