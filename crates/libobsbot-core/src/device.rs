// SPDX-License-Identifier: GPL-3.0-only
//! Opened camera handle and the v1 method surface.
//!
//! Each method routes through the crate-internal `Transport` trait. The
//! transport returns [`Error::Unsupported`] until real
//! `control_in`/`control_out` calls land; the method signatures and
//! entity/selector routing are stable.

use core::ops::RangeInclusive;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::devices::meet2;
use crate::discovery::DeviceInfo;
use crate::status::{Event, EventSender};
use crate::transport::Transport;
use crate::types::{
    AiMode, AutoFramingMode, Cadence, FovType, MediaMode, ProductType, Status, WdrMode,
    WhiteBalanceMode,
};
use crate::uvc::{self, UvcGet};
use crate::{Error, Result};

/// Opened OBSBOT camera.
///
/// Obtain via [`crate::Devices::open`]. Dropping the handle stops the
/// per-device status poller (joining its thread) and releases the
/// transport's `/dev/videoN` handle.
pub struct Device {
    info: DeviceInfo,
    transport: Arc<dyn Transport>,
    firmware: String,
    /// Poller period in milliseconds. Shared with the poller thread so
    /// [`Device::set_status_cadence`] can swap Slow/Fast on the fly.
    cadence_ms: Arc<AtomicU32>,
    /// Set by [`Device::drop`] to stop the poller.
    poller_stop: Arc<AtomicBool>,
    poller: Option<JoinHandle<()>>,
}

impl Device {
    pub(crate) fn new(
        info: DeviceInfo,
        transport: Arc<dyn Transport>,
        events_tx: Option<EventSender>,
    ) -> Self {
        let cadence_ms = Arc::new(AtomicU32::new(Cadence::Slow.period_ms()));
        let poller_stop = Arc::new(AtomicBool::new(false));
        let poller = events_tx.map(|tx| {
            spawn_status_poller(
                tx,
                info.serial.clone(),
                transport.clone(),
                cadence_ms.clone(),
                poller_stop.clone(),
            )
        });
        Self {
            info,
            transport,
            firmware: meet2::MIN_FW.to_owned(),
            cadence_ms,
            poller_stop,
            poller,
        }
    }

    /// Change how often the status poller samples the camera.
    pub fn set_status_cadence(&self, cadence: Cadence) {
        self.cadence_ms
            .store(cadence.period_ms(), Ordering::Relaxed);
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
    /// camera response can be parsed.
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
    ///
    /// The current-state fields (HDR / face-AE / AI mode / focus
    /// bits) come from a selector-0x02 RPC reply whose request frame
    /// hasn't been captured yet, so they remain `Default::default()`
    /// for now. Firmware / serial fields come from
    /// [`firmware_from_camera`](Self::firmware_from_camera) /
    /// [`serial_from_camera`](Self::serial_from_camera) and are
    /// filled in best-effort - they're left blank on read failure
    /// rather than failing the whole status call.
    pub fn status(&self) -> Result<Status> {
        let firmware = self.firmware_from_camera().unwrap_or_default();
        let serial = self.serial_from_camera().unwrap_or_default();
        let mut snap = sample_status(self.transport.as_ref());
        snap.firmware = firmware;
        snap.serial = serial;
        Ok(snap)
    }

    /// Ask the camera for its firmware version via the XU RPC channel.
    /// Sends a canned request frame captured from `libdev.so`; see
    /// `doc/protocol/meet2/crc-investigation.md` for why this is canned
    /// rather than freshly synthesised. Returns a dotted-decimal
    /// string like `"4.4.6.1"`.
    ///
    /// The canned request frame embeds the MAC of the captured Meet 2;
    /// other physical units will need their own canned frame until the
    /// CRC at offset 6-7 is decoded.
    pub fn firmware_from_camera(&self) -> Result<String> {
        self.rpc_request_then_reply(&meet2::RPC_REQUEST_FIRMWARE, meet2::decode_firmware_reply)
    }

    /// Ask the camera for its serial number via the XU RPC channel.
    /// Same canned-request caveat as [`firmware_from_camera`](Self::firmware_from_camera).
    pub fn serial_from_camera(&self) -> Result<String> {
        self.rpc_request_then_reply(&meet2::RPC_REQUEST_SERIAL, meet2::decode_serial_reply)
    }

    /// SET a canned `XU_SEL_RPC` request, then poll GET until `decode`
    /// returns a value. The camera processes SETs asynchronously, so
    /// the first GET right after a SET typically returns the previous
    /// session's reply.
    fn rpc_request_then_reply(
        &self,
        request: &[u8; meet2::RPC_FRAME_LEN],
        decode: impl Fn(&[u8]) -> Option<String>,
    ) -> Result<String> {
        self.transport
            .uvc_set(meet2::XU_ENTITY_ID, meet2::XU_SEL_RPC, request)?;
        let mut reply = [0u8; meet2::RPC_FRAME_LEN];
        for attempt in 0..meet2::RPC_REPLY_POLL_ATTEMPTS {
            // Tight loop, then back off; the camera typically catches up in
            // a few milliseconds.
            if attempt > 0 {
                std::thread::sleep(std::time::Duration::from_millis(
                    meet2::RPC_REPLY_POLL_DELAY_MS,
                ));
            }
            let _ = self.transport.uvc_get(
                UvcGet::Cur,
                meet2::XU_ENTITY_ID,
                meet2::XU_SEL_RPC,
                &mut reply,
            )?;
            if let Some(decoded) = decode(&reply) {
                return Ok(decoded);
            }
        }
        Err(Error::BadResponse {
            selector: meet2::XU_SEL_RPC,
            bytes: reply.to_vec(),
        })
    }

    // ---- Camera Terminal (standard UVC §A.9.4) ------------------------------

    /// Set pan and tilt in normalised camera coordinates (-1.0 ..= 1.0).
    ///
    /// Encodes as `CT_PANTILT_ABSOLUTE_CONTROL`: i32 LE pan + i32 LE tilt in
    /// arc-seconds (UVC 1.5 §4.2.2.1.14). The normalised-to-arc-second scale
    /// is provisional until the camera's `GET_MIN`/`GET_MAX` are queried at
    /// open time.
    #[allow(clippy::cast_possible_truncation)]
    pub fn set_pan_tilt(&self, pan: f32, tilt: f32) -> Result<()> {
        if !(-1.0..=1.0).contains(&pan) || !(-1.0..=1.0).contains(&tilt) {
            return Err(Error::OutOfRange);
        }
        let pan_i = (pan * PAN_TILT_PROVISIONAL_SCALE) as i32;
        let tilt_i = (tilt * PAN_TILT_PROVISIONAL_SCALE) as i32;
        let mut payload = [0u8; 8];
        payload[..4].copy_from_slice(&pan_i.to_le_bytes());
        payload[4..].copy_from_slice(&tilt_i.to_le_bytes());
        self.transport
            .uvc_set(uvc::CAMERA_TERMINAL, uvc::ct::PANTILT_ABSOLUTE, &payload)
    }

    /// Set zoom as a ratio of the camera's optical range (1.0 ..= max).
    ///
    /// Encodes as `CT_ZOOM_ABSOLUTE_CONTROL`: u16 LE objective focal length
    /// (UVC 1.5 §4.2.2.1.10). Mapping is provisional until `GET_MIN`/`GET_MAX`
    /// are queried.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn set_zoom(&self, zoom: f32) -> Result<()> {
        if !(0.0..=f32::from(u16::MAX)).contains(&zoom) {
            return Err(Error::OutOfRange);
        }
        let v = zoom as u16;
        self.transport.uvc_set(
            uvc::CAMERA_TERMINAL,
            uvc::ct::ZOOM_ABSOLUTE,
            &v.to_le_bytes(),
        )
    }

    /// Set focus distance.
    ///
    /// Encodes as `CT_FOCUS_ABSOLUTE_CONTROL`: u16 LE (UVC 1.5 §4.2.2.1.6).
    /// Mapping is provisional until `GET_MIN`/`GET_MAX` are queried.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn set_focus(&self, focus: f32) -> Result<()> {
        if !(0.0..=f32::from(u16::MAX)).contains(&focus) {
            return Err(Error::OutOfRange);
        }
        let v = focus as u16;
        self.transport.uvc_set(
            uvc::CAMERA_TERMINAL,
            uvc::ct::FOCUS_ABSOLUTE,
            &v.to_le_bytes(),
        )
    }

    /// Read current pan + tilt in normalised camera coordinates
    /// (-1.0 ..= 1.0). Inverse of [`set_pan_tilt`](Self::set_pan_tilt);
    /// uses the same provisional arc-second scale.
    pub fn pan_tilt(&self) -> Result<(f32, f32)> {
        let mut buf = [0u8; 8];
        let _ = self.transport.uvc_get(
            UvcGet::Cur,
            uvc::CAMERA_TERMINAL,
            uvc::ct::PANTILT_ABSOLUTE,
            &mut buf,
        )?;
        let pan = i32::from_le_bytes(buf[..4].try_into().unwrap());
        let tilt = i32::from_le_bytes(buf[4..].try_into().unwrap());
        Ok((normalised_pantilt(pan), normalised_pantilt(tilt)))
    }

    /// Read current zoom value. Returns the raw u16 as f32 until
    /// `GET_MIN`/`GET_MAX` are wired up to give a meaningful ratio.
    pub fn zoom(&self) -> Result<f32> {
        let mut buf = [0u8; 2];
        let _ = self.transport.uvc_get(
            UvcGet::Cur,
            uvc::CAMERA_TERMINAL,
            uvc::ct::ZOOM_ABSOLUTE,
            &mut buf,
        )?;
        Ok(f32::from(u16::from_le_bytes(buf)))
    }

    /// Read current focus value. Same provisional u16-as-f32 mapping
    /// as [`set_focus`](Self::set_focus).
    pub fn focus(&self) -> Result<f32> {
        let mut buf = [0u8; 2];
        let _ = self.transport.uvc_get(
            UvcGet::Cur,
            uvc::CAMERA_TERMINAL,
            uvc::ct::FOCUS_ABSOLUTE,
            &mut buf,
        )?;
        Ok(f32::from(u16::from_le_bytes(buf)))
    }

    // ---- Processing Unit (standard UVC §A.9.5) ------------------------------

    /// Set brightness. `PU_BRIGHTNESS_CONTROL`, i16 LE (UVC 1.5 §4.2.2.3.2).
    pub fn set_brightness(&self, value: i32) -> Result<()> {
        let v = i16::try_from(value).map_err(|_| Error::OutOfRange)?;
        self.transport
            .uvc_set(uvc::PROCESSING_UNIT, uvc::pu::BRIGHTNESS, &v.to_le_bytes())
    }

    /// Read current brightness.
    pub fn brightness(&self) -> Result<i32> {
        let mut buf = [0u8; 2];
        let _ = self.transport.uvc_get(
            UvcGet::Cur,
            uvc::PROCESSING_UNIT,
            uvc::pu::BRIGHTNESS,
            &mut buf,
        )?;
        Ok(i32::from(i16::from_le_bytes(buf)))
    }

    /// Reported brightness range.
    pub fn brightness_range(&self) -> Result<RangeInclusive<i32>> {
        let lo = self.pu_get_i16(UvcGet::Min, uvc::pu::BRIGHTNESS)?;
        let hi = self.pu_get_i16(UvcGet::Max, uvc::pu::BRIGHTNESS)?;
        Ok(i32::from(lo)..=i32::from(hi))
    }

    /// Set contrast. `PU_CONTRAST_CONTROL`, u16 LE (UVC 1.5 §4.2.2.3.3).
    pub fn set_contrast(&self, value: i32) -> Result<()> {
        let v = u16::try_from(value).map_err(|_| Error::OutOfRange)?;
        self.transport
            .uvc_set(uvc::PROCESSING_UNIT, uvc::pu::CONTRAST, &v.to_le_bytes())
    }

    /// Read current contrast.
    pub fn contrast(&self) -> Result<i32> {
        Ok(i32::from(self.pu_get_u16(UvcGet::Cur, uvc::pu::CONTRAST)?))
    }

    /// Reported contrast range.
    pub fn contrast_range(&self) -> Result<RangeInclusive<i32>> {
        let lo = self.pu_get_u16(UvcGet::Min, uvc::pu::CONTRAST)?;
        let hi = self.pu_get_u16(UvcGet::Max, uvc::pu::CONTRAST)?;
        Ok(i32::from(lo)..=i32::from(hi))
    }

    /// Set saturation. `PU_SATURATION_CONTROL`, u16 LE (UVC 1.5 §4.2.2.3.7).
    pub fn set_saturation(&self, value: i32) -> Result<()> {
        let v = u16::try_from(value).map_err(|_| Error::OutOfRange)?;
        self.transport
            .uvc_set(uvc::PROCESSING_UNIT, uvc::pu::SATURATION, &v.to_le_bytes())
    }

    /// Read current saturation.
    pub fn saturation(&self) -> Result<i32> {
        Ok(i32::from(
            self.pu_get_u16(UvcGet::Cur, uvc::pu::SATURATION)?,
        ))
    }

    /// Reported saturation range.
    pub fn saturation_range(&self) -> Result<RangeInclusive<i32>> {
        let lo = self.pu_get_u16(UvcGet::Min, uvc::pu::SATURATION)?;
        let hi = self.pu_get_u16(UvcGet::Max, uvc::pu::SATURATION)?;
        Ok(i32::from(lo)..=i32::from(hi))
    }

    // ---- White balance: hybrid (PU temperature, XU presets) -----------------

    /// Set white balance mode; the `kelvin` value is meaningful only when
    /// `mode` is [`WhiteBalanceMode::Manual`]. The Meet 2 only supports
    /// Auto and Manual (per the SDK header) so this routes entirely
    /// through standard UVC Processing Unit selectors `0x0a` / `0x0b`.
    pub fn set_white_balance(&self, mode: WhiteBalanceMode, kelvin: Option<u16>) -> Result<()> {
        match mode {
            WhiteBalanceMode::Auto => self.transport.uvc_set(
                uvc::PROCESSING_UNIT,
                uvc::pu::WHITE_BALANCE_TEMPERATURE_AUTO,
                &[1],
            ),
            WhiteBalanceMode::Manual => {
                self.transport.uvc_set(
                    uvc::PROCESSING_UNIT,
                    uvc::pu::WHITE_BALANCE_TEMPERATURE_AUTO,
                    &[0],
                )?;
                let k = kelvin.unwrap_or(6500);
                self.transport.uvc_set(
                    uvc::PROCESSING_UNIT,
                    uvc::pu::WHITE_BALANCE_TEMPERATURE,
                    &k.to_le_bytes(),
                )
            }
        }
    }

    /// Read current white-balance mode and Kelvin value.
    pub fn white_balance(&self) -> Result<(WhiteBalanceMode, u16)> {
        let mut auto_buf = [0u8; 1];
        let _ = self.transport.uvc_get(
            UvcGet::Cur,
            uvc::PROCESSING_UNIT,
            uvc::pu::WHITE_BALANCE_TEMPERATURE_AUTO,
            &mut auto_buf,
        )?;
        let kelvin = self.pu_get_u16(UvcGet::Cur, uvc::pu::WHITE_BALANCE_TEMPERATURE)?;
        let mode = if auto_buf[0] == 0 {
            WhiteBalanceMode::Manual
        } else {
            WhiteBalanceMode::Auto
        };
        Ok((mode, kelvin))
    }

    /// Reported manual Kelvin range.
    pub fn white_balance_range(&self) -> Result<RangeInclusive<u16>> {
        let lo = self.pu_get_u16(UvcGet::Min, uvc::pu::WHITE_BALANCE_TEMPERATURE)?;
        let hi = self.pu_get_u16(UvcGet::Max, uvc::pu::WHITE_BALANCE_TEMPERATURE)?;
        Ok(lo..=hi)
    }

    // ---- OBSBOT vendor extension (entity 2) ---------------------------------
    //
    // Every method below routes through the XU. Selector and payload layout
    // are pending per-method captures under doc/protocol/meet2/.

    /// Set HDR mode. Writes to the OBSBOT XU mode-register selector
    /// `0x06` with the WDR control id; see `doc/protocol/meet2/setWdr.md`
    /// for the wire format.
    pub fn set_wdr(&self, mode: WdrMode) -> Result<()> {
        let payload = meet2::mode_register_payload(meet2::MODE_WDR, &[encode_wdr(mode)]);
        self.transport
            .uvc_set(meet2::XU_ENTITY_ID, meet2::XU_SEL_MODE_REGISTER, &payload)
    }

    /// Read current HDR mode. GET reply format pending capture; the body
    /// reads one byte from the mode-register selector for now.
    pub fn wdr(&self) -> Result<WdrMode> {
        let mut buf = [0u8; 1];
        let _ = self.transport.uvc_get(
            UvcGet::Cur,
            meet2::XU_ENTITY_ID,
            meet2::XU_SEL_MODE_REGISTER,
            &mut buf,
        )?;
        decode_wdr(buf[0])
    }

    /// Set field-of-view preset. XU mode-register control id
    /// [`meet2::MODE_FOV`](crate::devices) - see
    /// `doc/protocol/meet2/setFov.md`.
    pub fn set_fov(&self, fov: FovType) -> Result<()> {
        let payload = meet2::mode_register_payload(meet2::MODE_FOV, &[encode_fov(fov)]);
        self.transport
            .uvc_set(meet2::XU_ENTITY_ID, meet2::XU_SEL_MODE_REGISTER, &payload)
    }

    /// Toggle face-based auto-exposure. XU mode-register control id
    /// `0x03` - see `doc/protocol/meet2/setFaceAE.md`.
    pub fn set_face_ae(&self, on: bool) -> Result<()> {
        let payload = meet2::mode_register_payload(meet2::MODE_FACE_AE, &[u8::from(on)]);
        self.transport
            .uvc_set(meet2::XU_ENTITY_ID, meet2::XU_SEL_MODE_REGISTER, &payload)
    }

    /// Toggle face-based auto-focus.
    ///
    /// Face-focus rides the XU selector-0x02 RPC channel
    /// (`cmd_set` 0x02, `cmd_id` 0x36) - not the simpler mode-register on
    /// 0x06. Two canned request frames are sent verbatim, one per
    /// state; see [`firmware_from_camera`](Self::firmware_from_camera)
    /// for the per-device caveat (the frames embed the captured Meet 2's
    /// MAC and CRC at offsets 6-7 / 14-15, both blocked on the CRC
    /// decode in `doc/protocol/meet2/crc-investigation.md`).
    pub fn set_face_focus(&self, on: bool) -> Result<()> {
        let request = if on {
            &meet2::RPC_REQUEST_FACE_FOCUS_ON
        } else {
            &meet2::RPC_REQUEST_FACE_FOCUS_OFF
        };
        self.transport
            .uvc_set(meet2::XU_ENTITY_ID, meet2::XU_SEL_RPC, request)
    }

    /// Select media mode. XU mode-register control id `0x00` - see
    /// `doc/protocol/meet2/setMediaMode.md`.
    pub fn set_media_mode(&self, mode: MediaMode) -> Result<()> {
        let payload =
            meet2::mode_register_payload(meet2::MODE_MEDIA_MODE, &[encode_media_mode(mode)]);
        self.transport
            .uvc_set(meet2::XU_ENTITY_ID, meet2::XU_SEL_MODE_REGISTER, &payload)
    }

    /// Set the auto-framing sub-mode. XU mode-register control id
    /// `0x0d` with a 2-byte value `[group_single, close_upper]`; see
    /// `doc/protocol/meet2/setAutoFraming.md`.
    pub fn set_auto_framing(&self, mode: AutoFramingMode) -> Result<()> {
        let payload =
            meet2::mode_register_payload(meet2::MODE_AUTO_FRAMING, &encode_auto_framing(mode));
        self.transport
            .uvc_set(meet2::XU_ENTITY_ID, meet2::XU_SEL_MODE_REGISTER, &payload)
    }

    /// Set the AI master mode. XU mode-register control id `0x16`
    /// with a u16 LE value; see `doc/protocol/meet2/setAiMode.md`.
    pub fn set_ai_mode(&self, mode: AiMode) -> Result<()> {
        let value: u16 = encode_ai_mode(mode);
        let payload = meet2::mode_register_payload(meet2::MODE_AI_MODE, &value.to_le_bytes());
        self.transport
            .uvc_set(meet2::XU_ENTITY_ID, meet2::XU_SEL_MODE_REGISTER, &payload)
    }

    // ---- private helpers ----------------------------------------------------

    fn pu_get_i16(&self, req: UvcGet, selector: u8) -> Result<i16> {
        let mut buf = [0u8; 2];
        let _ = self
            .transport
            .uvc_get(req, uvc::PROCESSING_UNIT, selector, &mut buf)?;
        Ok(i16::from_le_bytes(buf))
    }

    fn pu_get_u16(&self, req: UvcGet, selector: u8) -> Result<u16> {
        let mut buf = [0u8; 2];
        let _ = self
            .transport
            .uvc_get(req, uvc::PROCESSING_UNIT, selector, &mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }
}

// Pan/tilt arc-second scale used while the device's reported range hasn't
// been queried yet. 540_000 arc-seconds ≈ 150°, within the Meet 2's
// digital pan/tilt envelope. Real scale lands once GET_MIN/GET_MAX wire up.
const PAN_TILT_PROVISIONAL_SCALE: f32 = 540_000.0;

/// Wire-side arc-seconds -> normalised pan/tilt. The cast is fine for the
/// camera's actual range (~±540k); clippy's `cast_precision_loss` is a
/// red herring here since values outside f32's mantissa would be physical
/// nonsense.
#[allow(clippy::cast_precision_loss)]
fn normalised_pantilt(arc_seconds: i32) -> f32 {
    arc_seconds as f32 / PAN_TILT_PROVISIONAL_SCALE
}

/// Granularity for the poller's stop check between samples. The poll
/// thread sleeps in chunks this small so [`Device::drop`] can interrupt
/// it without waiting a full cadence period.
const POLLER_TICK: Duration = Duration::from_millis(25);

impl Drop for Device {
    fn drop(&mut self) {
        self.poller_stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.poller.take() {
            let _ = handle.join();
        }
    }
}

/// Spawn the per-device status poller thread. Reads brightness,
/// contrast, and saturation each cadence tick (the controls that have
/// stable wire formats today) and pushes a [`Event::Status`] into the
/// shared event channel. Exits when `stop` flips or the channel closes.
///
/// Firmware/serial are intentionally *not* sampled here. They ride the
/// XU RPC channel, take multiple round trips, and rarely change at
/// runtime; call [`Device::firmware_from_camera`] /
/// [`Device::serial_from_camera`] explicitly when you need them.
fn spawn_status_poller(
    tx: EventSender,
    serial: String,
    transport: Arc<dyn Transport>,
    cadence_ms: Arc<AtomicU32>,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name(format!("libobsbot-poll-{serial}"))
        .spawn(move || poll_loop(&tx, &serial, transport.as_ref(), &cadence_ms, &stop))
        .expect("spawn status poller thread")
}

fn poll_loop(
    tx: &EventSender,
    serial: &str,
    transport: &dyn Transport,
    cadence_ms: &AtomicU32,
    stop: &AtomicBool,
) {
    while !stop.load(Ordering::Relaxed) {
        let snapshot = sample_status(transport);
        if tx
            .send(Event::Status {
                serial: serial.to_owned(),
                snapshot,
            })
            .is_err()
        {
            return;
        }
        let target = Duration::from_millis(u64::from(cadence_ms.load(Ordering::Relaxed)));
        let mut slept = Duration::ZERO;
        while slept < target {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(POLLER_TICK);
            slept += POLLER_TICK;
        }
    }
}

/// One status sample. Reads what we have stable wire formats for; any
/// individual read that errors leaves the corresponding field at its
/// default rather than failing the whole sample.
fn sample_status(transport: &dyn Transport) -> Status {
    let mut snap = Status::default();
    if let Ok(v) = read_pu_i16(transport, uvc::pu::BRIGHTNESS) {
        snap.brightness = i32::from(v);
    }
    if let Ok(v) = read_pu_u16(transport, uvc::pu::CONTRAST) {
        snap.contrast = i32::from(v);
    }
    if let Ok(v) = read_pu_u16(transport, uvc::pu::SATURATION) {
        snap.saturation = i32::from(v);
    }
    let mut pt = [0u8; 8];
    if transport
        .uvc_get(
            UvcGet::Cur,
            uvc::CAMERA_TERMINAL,
            uvc::ct::PANTILT_ABSOLUTE,
            &mut pt,
        )
        .is_ok()
    {
        let pan = i32::from_le_bytes(pt[..4].try_into().unwrap());
        let tilt = i32::from_le_bytes(pt[4..].try_into().unwrap());
        snap.pan = normalised_pantilt(pan);
        snap.tilt = normalised_pantilt(tilt);
    }
    let mut zoom = [0u8; 2];
    if transport
        .uvc_get(
            UvcGet::Cur,
            uvc::CAMERA_TERMINAL,
            uvc::ct::ZOOM_ABSOLUTE,
            &mut zoom,
        )
        .is_ok()
    {
        snap.zoom = f32::from(u16::from_le_bytes(zoom));
    }
    snap
}

fn read_pu_i16(transport: &dyn Transport, selector: u8) -> Result<i16> {
    let mut buf = [0u8; 2];
    let _ = transport.uvc_get(UvcGet::Cur, uvc::PROCESSING_UNIT, selector, &mut buf)?;
    Ok(i16::from_le_bytes(buf))
}

fn read_pu_u16(transport: &dyn Transport, selector: u8) -> Result<u16> {
    let mut buf = [0u8; 2];
    let _ = transport.uvc_get(UvcGet::Cur, uvc::PROCESSING_UNIT, selector, &mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

// ---- payload helpers (XU encodings still placeholders) ---------------------

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
    // Matches the SDK enum and `setMediaMode.pcapng` frame 58.
    match mode {
        MediaMode::Normal => 0,
        MediaMode::Background => 1,
        MediaMode::AutoFrame => 2,
    }
}

/// 2-byte wire encoding `[group_single, close_upper]` for the
/// auto-framing sub-mode mode-register control.
fn encode_auto_framing(mode: AutoFramingMode) -> [u8; 2] {
    match mode {
        AutoFramingMode::Group => [0, 0],
        AutoFramingMode::SingleCloseUp => [1, 0],
        AutoFramingMode::SingleUpperBody => [1, 1],
    }
}

/// Maps our [`AiMode`] enum to the wire value seen in
/// `setAiMode.pcapng`. Matches the SDK's `Device::AiWorkModeType`.
fn encode_ai_mode(mode: AiMode) -> u16 {
    match mode {
        AiMode::None => 0,
        AiMode::Group => 1,
        AiMode::Human => 2,
        AiMode::Hand => 3,
        AiMode::WhiteBoard => 4,
        AiMode::Desk => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{device_with_mock, last_set};

    #[test]
    fn brightness_range_decodes_min_then_max() {
        use crate::testing::device_with_scripted_get;
        // Min = -64 (i16 LE: c0 ff), Max = 64 (i16 LE: 40 00).
        let device = device_with_scripted_get(vec![vec![0xc0, 0xff], vec![0x40, 0x00]]);
        let range = device.brightness_range().unwrap();
        assert_eq!(*range.start(), -64);
        assert_eq!(*range.end(), 64);
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
    fn pan_tilt_routes_to_camera_terminal() {
        let (d, mock) = device_with_mock();
        d.set_pan_tilt(0.0, 0.0).unwrap();
        let (entity, selector, payload) = last_set(&mock);
        assert_eq!(entity, uvc::CAMERA_TERMINAL);
        assert_eq!(selector, uvc::ct::PANTILT_ABSOLUTE);
        assert_eq!(payload.len(), 8);
    }

    #[test]
    fn brightness_routes_to_processing_unit_with_i16_payload() {
        let (d, mock) = device_with_mock();
        d.set_brightness(42).unwrap();
        let (entity, selector, payload) = last_set(&mock);
        assert_eq!(entity, uvc::PROCESSING_UNIT);
        assert_eq!(selector, uvc::pu::BRIGHTNESS);
        assert_eq!(payload, vec![42, 0]);
    }

    #[test]
    fn brightness_out_of_i16_range_is_refused() {
        let (d, _) = device_with_mock();
        assert!(matches!(
            d.set_brightness(i32::from(i16::MAX) + 1),
            Err(Error::OutOfRange)
        ));
    }

    #[test]
    fn contrast_and_saturation_route_to_pu() {
        let (d, mock) = device_with_mock();
        d.set_contrast(100).unwrap();
        let (entity, sel, payload) = last_set(&mock);
        assert_eq!(entity, uvc::PROCESSING_UNIT);
        assert_eq!(sel, uvc::pu::CONTRAST);
        assert_eq!(payload, vec![100, 0]);

        d.set_saturation(150).unwrap();
        let (entity, sel, payload) = last_set(&mock);
        assert_eq!(entity, uvc::PROCESSING_UNIT);
        assert_eq!(sel, uvc::pu::SATURATION);
        assert_eq!(payload, vec![150, 0]);
    }

    #[test]
    fn zoom_routes_to_camera_terminal_with_u16_payload() {
        let (d, mock) = device_with_mock();
        d.set_zoom(2.5).unwrap();
        let (entity, sel, payload) = last_set(&mock);
        assert_eq!(entity, uvc::CAMERA_TERMINAL);
        assert_eq!(sel, uvc::ct::ZOOM_ABSOLUTE);
        assert_eq!(payload, vec![2, 0]); // truncated u16 from 2.5
    }

    #[test]
    fn focus_routes_to_camera_terminal() {
        let (d, mock) = device_with_mock();
        d.set_focus(100.0).unwrap();
        let (entity, sel, _) = last_set(&mock);
        assert_eq!(entity, uvc::CAMERA_TERMINAL);
        assert_eq!(sel, uvc::ct::FOCUS_ABSOLUTE);
    }

    #[test]
    fn wb_auto_routes_to_pu() {
        let (d, mock) = device_with_mock();
        d.set_white_balance(WhiteBalanceMode::Auto, None).unwrap();
        let (entity, sel, payload) = last_set(&mock);
        assert_eq!(entity, uvc::PROCESSING_UNIT);
        assert_eq!(sel, uvc::pu::WHITE_BALANCE_TEMPERATURE_AUTO);
        assert_eq!(payload, vec![1]);
    }

    #[test]
    fn wb_manual_writes_kelvin_to_pu() {
        let (d, mock) = device_with_mock();
        d.set_white_balance(WhiteBalanceMode::Manual, Some(5500))
            .unwrap();
        let (entity, sel, payload) = last_set(&mock);
        assert_eq!(entity, uvc::PROCESSING_UNIT);
        assert_eq!(sel, uvc::pu::WHITE_BALANCE_TEMPERATURE);
        assert_eq!(payload, 5500_u16.to_le_bytes().to_vec());
    }

    #[test]
    fn wdr_routes_to_xu_mode_register_with_wire_bytes() {
        let (d, mock) = device_with_mock();
        d.set_wdr(WdrMode::Dol2To1).unwrap();
        let (entity, sel, payload) = last_set(&mock);
        assert_eq!(entity, meet2::XU_ENTITY_ID);
        assert_eq!(sel, meet2::XU_SEL_MODE_REGISTER);
        // setWdr.pcapng frame 70: control_id=0x01 (WDR), flag=0x01, value=0x01 (on).
        assert_eq!(payload.len(), meet2::MODE_REGISTER_PAYLOAD_LEN);
        assert_eq!(payload[..3], [0x01, 0x01, 0x01]);
        assert!(payload[3..].iter().all(|&b| b == 0));

        d.set_wdr(WdrMode::Off).unwrap();
        let (_, _, payload) = last_set(&mock);
        // setWdr.pcapng frame 82: control_id=0x01, flag=0x01, value=0x00 (off).
        assert_eq!(payload[..3], [0x01, 0x01, 0x00]);
    }

    #[test]
    fn auto_framing_routes_to_xu_mode_register_with_pair_value() {
        let (d, mock) = device_with_mock();

        // setAutoFramingGroup.pcapng: 0d 02 00 00
        d.set_auto_framing(AutoFramingMode::Group).unwrap();
        let (entity, sel, payload) = last_set(&mock);
        assert_eq!(entity, meet2::XU_ENTITY_ID);
        assert_eq!(sel, meet2::XU_SEL_MODE_REGISTER);
        assert_eq!(payload[..4], [0x0d, 0x02, 0x00, 0x00]);

        // setAutoFramingSingleCloseUp.pcapng: 0d 02 01 00
        d.set_auto_framing(AutoFramingMode::SingleCloseUp).unwrap();
        let (_, _, payload) = last_set(&mock);
        assert_eq!(payload[..4], [0x0d, 0x02, 0x01, 0x00]);

        // setAutoFramingSingleUpperBody.pcapng: 0d 02 01 01
        d.set_auto_framing(AutoFramingMode::SingleUpperBody)
            .unwrap();
        let (_, _, payload) = last_set(&mock);
        assert_eq!(payload[..4], [0x0d, 0x02, 0x01, 0x01]);
    }

    #[test]
    fn ai_mode_routes_to_xu_mode_register_with_u16_value() {
        // setAiMode.pcapng frame 56 (AI mode Human=2): 16 02 02 00 …
        let (d, mock) = device_with_mock();
        d.set_ai_mode(AiMode::Human).unwrap();
        let (entity, sel, payload) = last_set(&mock);
        assert_eq!(entity, meet2::XU_ENTITY_ID);
        assert_eq!(sel, meet2::XU_SEL_MODE_REGISTER);
        assert_eq!(payload[..4], [0x16, 0x02, 0x02, 0x00]);

        d.set_ai_mode(AiMode::None).unwrap();
        let (_, _, payload) = last_set(&mock);
        // setAiMode.pcapng frame 64: 16 02 00 00 …
        assert_eq!(payload[..4], [0x16, 0x02, 0x00, 0x00]);
    }

    #[test]
    fn fov_routes_to_xu_mode_register_with_wire_bytes() {
        // setFov.pcapng frame 52 (FovType78 = Medium): 04 01 01 00 …
        let (d, mock) = device_with_mock();
        d.set_fov(FovType::Medium).unwrap();
        let (entity, sel, payload) = last_set(&mock);
        assert_eq!(entity, meet2::XU_ENTITY_ID);
        assert_eq!(sel, meet2::XU_SEL_MODE_REGISTER);
        assert_eq!(payload.len(), meet2::MODE_REGISTER_PAYLOAD_LEN);
        assert_eq!(payload[..3], [0x04, 0x01, 0x01]);
        assert!(payload[3..].iter().all(|&b| b == 0));
    }

    #[test]
    fn media_mode_routes_to_xu_mode_register_with_wire_bytes() {
        // setMediaMode.pcapng frame 58 (MediaModeAutoFrame = 2): 00 01 02 …
        let (d, mock) = device_with_mock();
        d.set_media_mode(MediaMode::AutoFrame).unwrap();
        let (entity, sel, payload) = last_set(&mock);
        assert_eq!(entity, meet2::XU_ENTITY_ID);
        assert_eq!(sel, meet2::XU_SEL_MODE_REGISTER);
        assert_eq!(payload[..3], [0x00, 0x01, 0x02]);
    }

    #[test]
    fn face_ae_routes_to_xu_mode_register_with_wire_bytes() {
        // setFaceAE.pcapng frame 52 (on): 03 01 01 …
        let (d, mock) = device_with_mock();
        d.set_face_ae(true).unwrap();
        let (entity, sel, payload) = last_set(&mock);
        assert_eq!(entity, meet2::XU_ENTITY_ID);
        assert_eq!(sel, meet2::XU_SEL_MODE_REGISTER);
        assert_eq!(payload[..3], [0x03, 0x01, 0x01]);

        d.set_face_ae(false).unwrap();
        let (_, _, payload) = last_set(&mock);
        assert_eq!(payload[..3], [0x03, 0x01, 0x00]);
    }

    #[test]
    fn wdr_encode_decode_round_trip() {
        for mode in [WdrMode::Off, WdrMode::Dol2To1] {
            assert_eq!(decode_wdr(encode_wdr(mode)).unwrap(), mode);
        }
    }

    #[test]
    fn decode_rejects_unknown_byte() {
        assert!(matches!(decode_wdr(99), Err(Error::BadResponse { .. })));
    }

    #[test]
    fn pan_tilt_getter_decodes_two_i32_le() {
        use crate::testing::device_with_scripted_get;
        // 540_000 arc-seconds (= 1.0 after normalisation) is 0x0083d600.
        let mut buf = Vec::new();
        buf.extend_from_slice(&540_000_i32.to_le_bytes());
        buf.extend_from_slice(&(-270_000_i32).to_le_bytes());
        let device = device_with_scripted_get(vec![buf]);
        let (pan, tilt) = device.pan_tilt().unwrap();
        assert!((pan - 1.0).abs() < 1e-3);
        assert!((tilt + 0.5).abs() < 1e-3);
    }

    #[test]
    fn zoom_and_focus_getters_decode_u16_le() {
        use crate::testing::device_with_scripted_get;
        let device = device_with_scripted_get(vec![
            vec![0x10, 0x00], // zoom = 16
            vec![0xff, 0x00], // focus = 255
        ]);
        assert!((device.zoom().unwrap() - 16.0).abs() < f32::EPSILON);
        assert!((device.focus().unwrap() - 255.0).abs() < f32::EPSILON);
    }

    #[test]
    fn status_poller_emits_snapshots_when_sender_provided_and_stops_on_drop() {
        use crate::testing::{meet2_mock_info, Forward, MockTransport};

        let mock = Arc::new(MockTransport::default());
        let transport: Arc<dyn Transport> = Arc::new(Forward(mock));
        let (tx, rx) = crossbeam_channel::unbounded::<Event>();
        let device = Device::new(meet2_mock_info(), transport, Some(tx));
        device.set_status_cadence(Cadence::Fast);

        // The poller emits its first sample immediately on entry to the
        // loop, so a generous timeout here is purely defensive.
        let ev = rx
            .recv_timeout(Duration::from_millis(500))
            .expect("first status event");
        let Event::Status { snapshot, serial } = ev else {
            panic!("expected Status event, got {ev:?}");
        };
        assert_eq!(serial, "MOCK");
        // MockTransport zero-fills GETs, so every field reads as zero.
        assert_eq!(snapshot.brightness, 0);
        assert_eq!(snapshot.contrast, 0);
        assert_eq!(snapshot.saturation, 0);

        drop(device);

        // After drop, drain any in-flight samples then assert the
        // channel is quiet (poller exited).
        std::thread::sleep(Duration::from_millis(100));
        while rx.try_recv().is_ok() {}
        assert!(rx.try_recv().is_err(), "poller still emitting after drop");
    }
}
