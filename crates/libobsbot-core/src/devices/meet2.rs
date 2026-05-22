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

/// `XU_SEL_MODE_REGISTER` control id for the auto-framing sub-mode.
/// Value is two u8 bytes `[group_single, close_upper]` matching the
/// SDK's `Device::AutoFramingType` enum
/// (`group_single`: 0=Group / 1=Single; `close_upper`: 0=CloseUp /
/// 1=UpperBody, ignored when Group). `setAutoFraming*.pcapng`.
pub(crate) const MODE_AUTO_FRAMING: u8 = 0x0d;

/// Payload length for every `XU_SEL_MODE_REGISTER` SET observed so far.
pub(crate) const MODE_REGISTER_PAYLOAD_LEN: usize = 60;

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

/// Canned `XU_SEL_RPC` SET request that asks the camera for its
/// firmware version. Captured at `doc/protocol/meet2/getStatus.pcapng`
/// frame 28. Includes the CRC at offset 6-7 from the capture.
///
/// **Device-specific:** the MAC tail at offset 18-23
/// (`ad b6 1b 98 dc 8d`) belongs to the captured unit. Until the CRC
/// is cracked (see `doc/protocol/meet2/crc-investigation.md`), the
/// canned bytes only address that one device; another Meet 2 would
/// produce different CRC bytes and require a fresh capture.
pub(crate) const RPC_REQUEST_FIRMWARE: [u8; RPC_FRAME_LEN] = [
    0xaa, 0x01, 0x01, 0x00, 0x0c, 0x00, 0xc1, 0x50, 0x0a, 0x0d, 0x08, 0x04, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0xad, 0xb6, 0x1b, 0x98, 0xdc, 0x8d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// Canned `XU_SEL_RPC` SET request that asks the camera for its
/// serial number. Captured at `doc/protocol/meet2/getStatus.pcapng`
/// frame 44. Same device-specific caveat as [`RPC_REQUEST_FIRMWARE`].
pub(crate) const RPC_REQUEST_SERIAL: [u8; RPC_FRAME_LEN] = [
    0xaa, 0x01, 0x03, 0x00, 0x0c, 0x00, 0x31, 0x53, 0x0a, 0x0d, 0xc8, 0x18, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xad, 0xb6, 0x1b, 0x98, 0xdc, 0x8d, 0x01, 0x01,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// Canned `XU_SEL_RPC` SET frame that enables face-based auto-focus.
/// Captured from `setFaceFocusOn.pcapng` frame 52. `cmd_set` 0x02,
/// `cmd_id` 0x36, 4-byte payload `[0x01, 0x00, 0x00, 0x00]`. Same
/// device-specific caveat as [`RPC_REQUEST_FIRMWARE`].
pub(crate) const RPC_REQUEST_FACE_FOCUS_ON: [u8; RPC_FRAME_LEN] = [
    0xaa, 0x25, 0x04, 0x00, 0x0c, 0x00, 0xd8, 0xc6, 0x0a, 0x02, 0x02, 0x36, 0x04, 0x00, 0xbf, 0xfb,
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xad, 0xb6, 0x1b, 0x98, 0xdc, 0x8d,
    0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// Canned `XU_SEL_RPC` SET frame that disables face-based auto-focus.
/// Captured from `setFaceFocusOff.pcapng` frame 52. Same payload
/// shape as [`RPC_REQUEST_FACE_FOCUS_ON`] with value `[0x00, 0x00,
/// 0x00, 0x00]`.
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
