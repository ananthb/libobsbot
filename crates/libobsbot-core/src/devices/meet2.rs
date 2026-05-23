// SPDX-License-Identifier: GPL-3.0-only
//! OBSBOT Meet 2 command table.
//!
//! Every constant in this module MUST be justified by a committed pcap under
//! `doc/protocol/meet2/`, or - for the camera-level identifiers below - by
//! the committed `descriptors.txt` dump of `lsusb -v -d 3564:fefb`. See
//! `CONTRIBUTING.md` for the sourcing rule.

/// OBSBOT USB vendor id (Remo Tech Co., Ltd.).
pub(crate) const VENDOR_ID: u16 = 0x3564;

/// OBSBOT Meet 2 USB product id.
pub(crate) const PRODUCT_ID_MEET2: u16 = 0xfefb;

/// Extension Unit entity id for the OBSBOT vendor XU on the Meet 2.
///
/// The Meet 2's XU has GUID `{9a1e7291-6843-4683-6d92-39bc7906ee49}` and
/// exposes 7 controls. The GUID itself is documented in
/// `doc/protocol/meet2/README.md` and `descriptors.txt`; we identify the
/// device by `(vendor_id, product_id)` and address the XU by entity id.
pub(crate) const XU_ENTITY_ID: u8 = 2;

/// XU "mode register" selector: 60-byte SET with the layout
/// `[control_id, 0x01, value, 0x00 × 57]`. Each known control id maps
/// to one OBSBOT proprietary on/off-or-enum control.
///
/// Refs: `doc/protocol/meet2/setWdr.pcapng`,
///       `doc/protocol/meet2/setWdr.md`.
pub(crate) const XU_SEL_MODE_REGISTER: u8 = 0x06;

/// `XU_SEL_MODE_REGISTER` control id for media mode. Value byte matches
/// the SDK's `Device::MediaMode` enum (0=Normal, 1=Background, 2=AutoFrame).
/// `setMediaMode.pcapng` frame 58 (value=2).
pub(crate) const MODE_MEDIA_MODE: u8 = 0x00;

/// `XU_SEL_MODE_REGISTER` control id for WDR / HDR. Value byte: `1` =
/// `DOL2-to-1` HDR on, `0` = HDR off. `setWdr.pcapng` frames 70 + 82.
pub(crate) const MODE_WDR: u8 = 0x01;

/// `XU_SEL_MODE_REGISTER` control id for face-based auto-exposure.
/// Value byte: `0` = off, `1` = on. `setFaceAE.pcapng` frame 52.
pub(crate) const MODE_FACE_AE: u8 = 0x03;

/// `XU_SEL_MODE_REGISTER` control id for FOV preset. Value byte matches
/// the SDK's `Device::FovType` enum (0=86°/Wide, 1=78°/Medium,
/// 2=65°/Narrow). `setFov.pcapng` frame 52 (value=1).
pub(crate) const MODE_FOV: u8 = 0x04;

/// `XU_SEL_MODE_REGISTER` control id for AI master mode. Value is a
/// u16 LE matching the SDK's `Device::AiWorkModeType` enum (0=None,
/// 1=Group, 2=Human, 3=Hand, 4=WhiteBoard, 5=Desk).
/// `setAiMode.pcapng` frames 56 (value=2) + 64 (value=0).
pub(crate) const MODE_AI_MODE: u8 = 0x16;

/// `XU_SEL_MODE_REGISTER` control id for the microphone Automatic
/// Gain Control. Value byte: `0` = off, `1` = on. Recovered from
/// `libdev.so::cameraSetAudioAGC` (catch-all branch calls
/// `uvcExtSet(selector=0x06, [0x17, 0x01, value, 0×57])`).
pub(crate) const MODE_AUDIO_AGC: u8 = 0x17;

/// `XU_SEL_MODE_REGISTER` control id for portrait-mode orientation.
/// Value byte: `0` = landscape (default), `1` = portrait (90°
/// rotation). Recovered from `libdev.so::cameraSetVerticalModeU`
/// (always calls `uvcExtSet(0x06, [0x0c, 0x01, value, ...])` regardless
/// of `productType`).
pub(crate) const MODE_VERTICAL: u8 = 0x0c;

/// `XU_SEL_MODE_REGISTER` control id for horizontal image flip
/// (left/right mirror). Value byte: `0` = off, `1` = on. Recovered
/// from `libdev.so::cameraSetImageFlipHorizonU`.
pub(crate) const MODE_FLIP_HORIZONTAL: u8 = 0x14;

/// `XU_SEL_MODE_REGISTER` control id for the camera's front-facing
/// status LED. Value byte: `0` = off, `1` = on. Recovered from
/// `libdev.so::cameraSetLedCtrlU`.
pub(crate) const MODE_LED: u8 = 0x18;

/// `XU_SEL_MODE_REGISTER` control id for the auto-framing sub-mode.
/// Value is two u8 bytes `[group_single, close_upper]` matching the
/// SDK's `Device::AutoFramingType` enum
/// (`group_single`: 0=Group / 1=Single; `close_upper`: 0=CloseUp /
/// 1=UpperBody, ignored when Group). `setAutoFraming*.pcapng`.
pub(crate) const MODE_AUTO_FRAMING: u8 = 0x0d;

/// Payload length for every `XU_SEL_MODE_REGISTER` SET observed so far.
pub(crate) const MODE_REGISTER_PAYLOAD_LEN: usize = 60;

/// First byte of the 60-byte status blob the camera returns on a
/// `GET_CUR` of `XU_SEL_MODE_REGISTER`. Hard-coded by firmware 4.4.6.1;
/// re-verify on firmware updates.
pub(crate) const STATUS_BLOB_MARKER: u8 = 0x27;

/// Offset in the status blob of the WDR byte (`0` off / `1` on).
pub(crate) const STATUS_WDR_OFFSET: usize = 6;

/// Offset in the status blob of the face-AE byte (`0` off / `1` on).
pub(crate) const STATUS_FACE_AE_OFFSET: usize = 7;

/// Offset in the status blob of the AI master-mode byte. Matches the
/// `AiMode` enum (0=None, 1=Group, 2=Human, 3=Hand, 4=WhiteBoard,
/// 5=Desk). Setting `MediaMode::AutoFrame` or any `AutoFramingMode`
/// updates this same byte to the equivalent AI mode value.
pub(crate) const STATUS_AI_MODE_OFFSET: usize = 0x18;

/// Build a payload for `XU_SEL_MODE_REGISTER` from
/// `(control_id, value_bytes)`. Layout:
/// `[control_id, value_bytes.len() as u8, value_bytes..., 0x00 padding]`.
/// `value_bytes` must fit in the 58 remaining bytes; the WDR / FOV /
/// faceAE / mediaMode controls use 1 byte, AI mode uses 2.
pub(crate) fn mode_register_payload(
    control_id: u8,
    value_bytes: &[u8],
) -> [u8; MODE_REGISTER_PAYLOAD_LEN] {
    let mut buf = [0u8; MODE_REGISTER_PAYLOAD_LEN];
    buf[0] = control_id;
    buf[1] = u8::try_from(value_bytes.len()).expect("mode-register value must fit in 255 bytes");
    buf[2..2 + value_bytes.len()].copy_from_slice(value_bytes);
    buf
}

/// Minimum firmware version this build supports.
/// Updated once the first hardware verification run lands.
pub(crate) const MIN_FW: &str = "0.0.0";

/// XU "RPC channel" selector. 60-byte SET/GET pairs carry an
/// OBSBOT-proprietary command framing; see
/// `doc/protocol/meet2/getStatus.md`.
pub(crate) const XU_SEL_RPC: u8 = 0x02;

/// Length of every `XU_SEL_RPC` SET request and GET reply.
pub(crate) const RPC_FRAME_LEN: usize = 60;

/// How many times to poll the `XU_SEL_RPC` GET buffer before giving up
/// on a request, since the camera processes our SET asynchronously.
pub(crate) const RPC_REPLY_POLL_ATTEMPTS: u32 = 10;

/// Delay between `XU_SEL_RPC` GET polls in milliseconds.
pub(crate) const RPC_REPLY_POLL_DELAY_MS: u64 = 20;

/// Offset of the `cmd_id` byte inside an `XU_SEL_RPC` frame.
const RPC_CMD_ID_OFFSET: usize = 10;
/// Offset of the `sub_cmd_id` byte.
const RPC_SUB_CMD_ID_OFFSET: usize = 11;
/// Offset of the little-endian u16 payload length.
const RPC_LEN_OFFSET: usize = 12;
/// Offset where the variable-length payload starts.
const RPC_PAYLOAD_OFFSET: usize = 16;

/// `XU_SEL_RPC` reply tuple `(cmd_id, sub_cmd_id)` that returns the
/// 4-byte firmware version. `getStatus.pcapng` frame 33.
const RPC_GET_FIRMWARE: (u8, u8) = (0x08, 0x04);

/// `XU_SEL_RPC` reply tuple that returns the device serial as up to
/// 14 ASCII bytes (NUL-padded). `getStatus.pcapng` frame 49.
const RPC_GET_SERIAL: (u8, u8) = (0xC8, 0x18);

/// MAC tail of the originally captured Meet 2. Used only by unit
/// tests (real opens learn the MAC from the camera via
/// [`build_mac_query_request`]).
#[cfg(test)]
pub(crate) const CAPTURED_MAC: [u8; 6] = [0xad, 0xb6, 0x1b, 0x98, 0xdc, 0x8d];

/// Pinned test fixture: the captured firmware-request frame.
/// Regenerated on demand by [`build_rpc_frame`] with the captured MAC.
/// Kept as a `const` so unit tests can verify byte-for-byte that the
/// builder still reproduces what `getStatus.pcapng` frame 28 showed.
#[cfg(test)]
pub(crate) const RPC_REQUEST_FIRMWARE: [u8; RPC_FRAME_LEN] = [
    0xaa, 0x01, 0x01, 0x00, 0x0c, 0x00, 0xc1, 0x50, 0x0a, 0x0d, 0x08, 0x04, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0xad, 0xb6, 0x1b, 0x98, 0xdc, 0x8d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// Pinned test fixture: the captured serial-request frame.
#[cfg(test)]
pub(crate) const RPC_REQUEST_SERIAL: [u8; RPC_FRAME_LEN] = [
    0xaa, 0x01, 0x03, 0x00, 0x0c, 0x00, 0x31, 0x53, 0x0a, 0x0d, 0xc8, 0x18, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xad, 0xb6, 0x1b, 0x98, 0xdc, 0x8d, 0x01, 0x01,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// Pinned test fixture: the captured face-focus-on frame.
#[cfg(test)]
pub(crate) const RPC_REQUEST_FACE_FOCUS_ON: [u8; RPC_FRAME_LEN] = [
    0xaa, 0x25, 0x04, 0x00, 0x0c, 0x00, 0xd8, 0xc6, 0x0a, 0x02, 0x02, 0x36, 0x04, 0x00, 0xbf, 0xfb,
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xad, 0xb6, 0x1b, 0x98, 0xdc, 0x8d,
    0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// Pinned test fixture: the captured face-focus-off frame.
#[cfg(test)]
pub(crate) const RPC_REQUEST_FACE_FOCUS_OFF: [u8; RPC_FRAME_LEN] = [
    0xaa, 0x25, 0x04, 0x00, 0x0c, 0x00, 0xd8, 0xc6, 0x0a, 0x02, 0x02, 0x36, 0x04, 0x00, 0xbe, 0x07,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xad, 0xb6, 0x1b, 0x98, 0xdc, 0x8d,
    0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// Decode an `XU_SEL_RPC` GET reply that's expected to carry the
/// firmware version.
pub(crate) fn decode_firmware_reply(buf: &[u8]) -> Option<String> {
    let (cmd_id, sub_cmd_id, payload) = parse_rpc_reply(buf)?;
    if (cmd_id, sub_cmd_id) != RPC_GET_FIRMWARE || payload.len() < 4 {
        return None;
    }
    Some(format!(
        "{}.{}.{}.{}",
        payload[3], payload[2], payload[1], payload[0]
    ))
}

/// Decode an `XU_SEL_RPC` GET reply that's expected to carry the
/// device serial.
pub(crate) fn decode_serial_reply(buf: &[u8]) -> Option<String> {
    let (cmd_id, sub_cmd_id, payload) = parse_rpc_reply(buf)?;
    if (cmd_id, sub_cmd_id) != RPC_GET_SERIAL {
        return None;
    }
    let end = payload
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(payload.len());
    let bytes = &payload[..end];
    if bytes.iter().any(|&b| !b.is_ascii_graphic()) {
        return None;
    }
    Some(core::str::from_utf8(bytes).ok()?.to_owned())
}

/// CRC-16/USB (poly `0x8005`, init `0xFFFF`, refin/refout=true,
/// xorout=`0xFFFF`). This is the algorithm `libdev.so::calc_crc16`
/// implements via its `crc16_low_tab` / `crc16_high_tab` lookup
/// tables; see `doc/protocol/meet2/crc-investigation.md` for the
/// disassembly trace that nailed it.
pub(crate) fn crc16_usb(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &b in data {
        crc ^= u16::from(b);
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// Build an `XU_SEL_RPC` request frame from its logical pieces, computing
/// the outer CRC (and the inner one when bit 5 or 6 of `seq_byte` is
/// set) the same way `libdev.so::frmHeaderProcessForSendV3` does.
///
/// Outer header layout (12 bytes covered by the outer CRC at `[6,7]`):
///
/// ```text
/// offset 0:     0xAA               magic
/// offset 1:     `seq_byte`         (libdev forces the low 10 bits to
///                                  0x1AA; we accept the full byte from
///                                  the original capture)
/// offset 2:     `sub_seq`          increments per (request, reply) pair
/// offset 3:     0x00               reserved
/// offset 4-5:   `outer_len`        u16 LE; libdev always writes 12
/// offset 6-7:   outer CRC          filled in by this helper
/// offset 8:     0x0A               request direction marker
/// offset 9:     `cmd_set`          varies per command family
/// offset 10:    `cmd_id`
/// offset 11:    `sub_cmd_id`
/// ```
///
/// Inner section (covered by the inner CRC at `[14,15]` when
/// `seq_byte & 0x60 != 0`):
///
/// ```text
/// offset 12-13: `payload.len()` u16 LE
/// offset 14-15: inner CRC (zeroed during CRC computation)
/// offset 16..:  payload bytes
/// ```
///
/// Everything past the inner section is left at whatever the caller
/// supplies in `tail`. That's where the device-specific MAC and any
/// per-command sentinel bytes live; the camera doesn't validate them
/// via CRC, but it does seem to require the right MAC at the right
/// offset for some commands.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_rpc_frame(
    seq_byte: u8,
    sub_seq: u8,
    cmd_set: u8,
    cmd_id: u8,
    sub_cmd_id: u8,
    payload: &[u8],
    tail_offset: usize,
    tail: &[u8],
) -> [u8; RPC_FRAME_LEN] {
    assert!(payload.len() + 16 <= RPC_FRAME_LEN, "payload too long");
    assert!(tail_offset >= 16 + payload.len(), "tail overlaps payload");
    assert!(tail_offset + tail.len() <= RPC_FRAME_LEN, "tail past frame");
    let mut buf = [0u8; RPC_FRAME_LEN];
    buf[0] = 0xAA;
    buf[1] = seq_byte;
    buf[2] = sub_seq;
    let outer_len: u16 = 12;
    buf[4..6].copy_from_slice(&outer_len.to_le_bytes());
    buf[8] = 0x0A;
    buf[9] = cmd_set;
    buf[10] = cmd_id;
    buf[11] = sub_cmd_id;
    let inner_len = u16::try_from(payload.len()).expect("payload fits in u16");
    buf[12..14].copy_from_slice(&inner_len.to_le_bytes());
    buf[16..16 + payload.len()].copy_from_slice(payload);
    buf[tail_offset..tail_offset + tail.len()].copy_from_slice(tail);

    let outer = crc16_usb(&buf[..usize::from(outer_len)]);
    buf[6..8].copy_from_slice(&outer.to_le_bytes());

    if seq_byte & 0x60 != 0 {
        let inner_end = 16 + payload.len();
        let inner = crc16_usb(&buf[12..inner_end]);
        buf[14..16].copy_from_slice(&inner.to_le_bytes());
    }

    buf
}

/// Build the `(cmd_id, sub_cmd_id) = (0x08, 0x18)` handshake request
/// that asks the camera for its 24-byte device hash. The MAC tail of
/// that hash is what other RPC commands need to embed; this request
/// itself takes no MAC, so it's the one bootstrap frame we can build
/// on a freshly-opened camera with no prior state.
pub(crate) fn build_mac_query_request() -> [u8; RPC_FRAME_LEN] {
    build_rpc_frame(0x01, 0x00, 0x0D, 0x08, 0x18, &[], 16, &[])
}

/// `(cmd_id, sub_cmd_id)` of the device-hash reply. The reply payload
/// is 24 bytes; bytes 18-23 of the payload are the MAC tail.
const RPC_GET_DEVICE_HASH: (u8, u8) = (0x08, 0x18);

/// Pull the MAC tail out of an `XU_SEL_RPC` GET reply. Returns `None`
/// unless the reply matches `RPC_GET_DEVICE_HASH` and is long enough.
pub(crate) fn decode_mac_query_reply(buf: &[u8]) -> Option<[u8; 6]> {
    let (cmd_id, sub_cmd_id, payload) = parse_rpc_reply(buf)?;
    if (cmd_id, sub_cmd_id) != RPC_GET_DEVICE_HASH || payload.len() < 24 {
        return None;
    }
    payload[18..24].try_into().ok()
}

/// Pull (`cmd_id`, `sub_cmd_id`, payload) out of an `XU_SEL_RPC` GET
/// reply buffer. Returns `None` if the magic or direction marker
/// doesn't match a reply.
fn parse_rpc_reply(buf: &[u8]) -> Option<(u8, u8, &[u8])> {
    if buf.len() < RPC_PAYLOAD_OFFSET || buf[0] != 0xAA {
        return None;
    }
    // Reply direction marker: 0x0D, 0x0A. Requests have these flipped.
    if buf[8] != 0x0D || buf[9] != 0x0A {
        return None;
    }
    let cmd_id = buf[RPC_CMD_ID_OFFSET];
    let sub_cmd_id = buf[RPC_SUB_CMD_ID_OFFSET];
    let len = u16::from_le_bytes([buf[RPC_LEN_OFFSET], buf[RPC_LEN_OFFSET + 1]]) as usize;
    let end = RPC_PAYLOAD_OFFSET.checked_add(len)?;
    if end > buf.len() {
        return None;
    }
    Some((cmd_id, sub_cmd_id, &buf[RPC_PAYLOAD_OFFSET..end]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_register_payload_layout_matches_wire_one_byte() {
        // setWdr.pcapng frame 70 (HDR on): 01 01 01 00 …
        let on = mode_register_payload(MODE_WDR, &[1]);
        assert_eq!(on[..3], [0x01, 0x01, 0x01]);
        assert!(on[3..].iter().all(|&b| b == 0));
        assert_eq!(on.len(), 60);

        // setWdr.pcapng frame 82 (HDR off): 01 01 00 00 …
        let off = mode_register_payload(MODE_WDR, &[0]);
        assert_eq!(off[..3], [0x01, 0x01, 0x00]);
        assert!(off[3..].iter().all(|&b| b == 0));
    }

    #[test]
    fn crc16_usb_matches_known_vectors() {
        // CRC-16/USB("123456789") = 0xB4C8 per the CRC catalog.
        assert_eq!(crc16_usb(b"123456789"), 0xB4C8);
        // Zero-byte input: CRC starts at 0xFFFF; final XOR (NOT) is 0x0000.
        assert_eq!(crc16_usb(b""), 0x0000);
    }

    #[test]
    fn build_rpc_frame_reproduces_canned_firmware_request() {
        // RPC_REQUEST_FIRMWARE: seq=0x01 sub=0x01 cmd_set=0x0D
        // cmd=0x08 sub_cmd=0x04 no payload, MAC at [18..24].
        let mac = [0xad, 0xb6, 0x1b, 0x98, 0xdc, 0x8d];
        let built = build_rpc_frame(0x01, 0x01, 0x0D, 0x08, 0x04, &[], 18, &mac);
        assert_eq!(built, RPC_REQUEST_FIRMWARE);
    }

    #[test]
    fn build_rpc_frame_reproduces_canned_serial_request() {
        // RPC_REQUEST_SERIAL: seq=0x01 sub=0x03 cmd_set=0x0D
        // cmd=0xC8 sub_cmd=0x18 no payload, MAC at [24..30] + sentinel
        // `01 01` at [30..32].
        let mut tail = [0u8; 8];
        tail[..6].copy_from_slice(&[0xad, 0xb6, 0x1b, 0x98, 0xdc, 0x8d]);
        tail[6] = 0x01;
        tail[7] = 0x01;
        let built = build_rpc_frame(0x01, 0x03, 0x0D, 0xC8, 0x18, &[], 24, &tail);
        assert_eq!(built, RPC_REQUEST_SERIAL);
    }

    #[test]
    fn build_rpc_frame_reproduces_canned_face_focus_on() {
        // RPC_REQUEST_FACE_FOCUS_ON: seq=0x25 (triggers inner CRC since
        // 0x25 & 0x60 = 0x20), sub=0x04, cmd_set=0x02 cmd=0x02 sub_cmd=0x36
        // payload=[01,00,00,00], MAC at [26..32], sentinel `01 01` at [32,33].
        let payload = [0x01, 0x00, 0x00, 0x00];
        let mut tail = [0u8; 8];
        tail[..6].copy_from_slice(&[0xad, 0xb6, 0x1b, 0x98, 0xdc, 0x8d]);
        tail[6] = 0x01;
        tail[7] = 0x01;
        let built = build_rpc_frame(0x25, 0x04, 0x02, 0x02, 0x36, &payload, 26, &tail);
        assert_eq!(built, RPC_REQUEST_FACE_FOCUS_ON);
    }

    #[test]
    fn build_rpc_frame_reproduces_canned_face_focus_off() {
        let payload = [0x00, 0x00, 0x00, 0x00];
        let mut tail = [0u8; 8];
        tail[..6].copy_from_slice(&[0xad, 0xb6, 0x1b, 0x98, 0xdc, 0x8d]);
        tail[6] = 0x01;
        tail[7] = 0x01;
        let built = build_rpc_frame(0x25, 0x04, 0x02, 0x02, 0x36, &payload, 26, &tail);
        assert_eq!(built, RPC_REQUEST_FACE_FOCUS_OFF);
    }

    #[test]
    fn mode_register_payload_layout_matches_wire_two_byte() {
        // setAiMode.pcapng frame 56 (AI mode Human=2): 16 02 02 00 …
        let on = mode_register_payload(MODE_AI_MODE, &2u16.to_le_bytes());
        assert_eq!(on[..4], [0x16, 0x02, 0x02, 0x00]);
        assert!(on[4..].iter().all(|&b| b == 0));

        // setAiMode.pcapng frame 64 (AI mode None=0): 16 02 00 00 …
        let off = mode_register_payload(MODE_AI_MODE, &0u16.to_le_bytes());
        assert_eq!(off[..4], [0x16, 0x02, 0x00, 0x00]);
        assert!(off[4..].iter().all(|&b| b == 0));
    }
}
