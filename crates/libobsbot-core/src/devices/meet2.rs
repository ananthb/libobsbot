// SPDX-License-Identifier: GPL-3.0-only
//! OBSBOT Meet 2 command table.
//!
//! Every constant in this module MUST be justified by a committed pcap under
//! `docs/protocol/meet2/`, or — for the camera-level identifiers below — by
//! the committed `descriptors.txt` dump of `lsusb -v -d 3564:fefb`. See
//! `CONTRIBUTING.md` for the sourcing rule.

/// OBSBOT USB vendor id (Remo Tech Co., Ltd.).
pub(crate) const VENDOR_ID: u16 = 0x3564;

/// OBSBOT Meet 2 USB product id.
pub(crate) const PRODUCT_ID_MEET2: u16 = 0xfefb;

/// `VideoControl` interface number that owns the extension unit.
pub(crate) const VIDEO_CONTROL_INTERFACE: u8 = 0;

/// Extension Unit ID for the OBSBOT vendor XU on the Meet 2.
pub(crate) const XU_ENTITY_ID: u8 = 2;

/// Raw 16-byte XU GUID as it appears in the descriptor.
/// Microsoft GUID encoding: first three fields little-endian, last eight bytes
/// big-endian. Renders as `{9a1e7291-6843-4683-6d92-39bc7906ee49}`.
pub(crate) const XU_GUID: [u8; 16] = [
    0x91, 0x72, 0x1e, 0x9a, 0x43, 0x68, 0x83, 0x46, 0x6d, 0x92, 0x39, 0xbc, 0x79, 0x06, 0xee, 0x49,
];

/// Minimum firmware version this build supports.
/// `bcdDevice` reads 5.10 on the unit used for capture; the camera firmware
/// version proper comes from a status read once that selector is captured.
pub(crate) const MIN_FW: &str = "0.0.0";
