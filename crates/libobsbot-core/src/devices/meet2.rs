// SPDX-License-Identifier: GPL-3.0-only
//! OBSBOT Meet 2 command table.
//!
//! Every constant in this module MUST be justified by a committed pcap under
//! `doc/protocol/meet2/`, or — for the camera-level identifiers below — by
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

/// Payload length for every `XU_SEL_MODE_REGISTER` SET observed so far.
pub(crate) const MODE_REGISTER_PAYLOAD_LEN: usize = 60;

/// Build a payload for `XU_SEL_MODE_REGISTER` from
/// `(control_id, value)`. Layout: `[control_id, 0x01, value, 0x00 × 57]`.
pub(crate) fn mode_register_payload(control_id: u8, value: u8) -> [u8; MODE_REGISTER_PAYLOAD_LEN] {
    let mut buf = [0u8; MODE_REGISTER_PAYLOAD_LEN];
    buf[0] = control_id;
    buf[1] = 0x01;
    buf[2] = value;
    buf
}

/// Minimum firmware version this build supports.
/// Updated once the first hardware verification run lands.
pub(crate) const MIN_FW: &str = "0.0.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_register_payload_layout_matches_wire() {
        // setWdr.pcapng frame 70 (HDR on): 01 01 01 00 …
        let on = mode_register_payload(MODE_WDR, 1);
        assert_eq!(on[..3], [0x01, 0x01, 0x01]);
        assert!(on[3..].iter().all(|&b| b == 0));
        assert_eq!(on.len(), 60);

        // setWdr.pcapng frame 82 (HDR off): 01 01 00 00 …
        let off = mode_register_payload(MODE_WDR, 0);
        assert_eq!(off[..3], [0x01, 0x01, 0x00]);
        assert!(off[3..].iter().all(|&b| b == 0));
    }
}
