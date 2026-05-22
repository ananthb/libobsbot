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

/// Extension Unit entity id for the OBSBOT vendor XU on the Meet 2.
///
/// The Meet 2's XU has GUID `{9a1e7291-6843-4683-6d92-39bc7906ee49}` and
/// exposes 7 controls. The GUID itself is documented in
/// `docs/protocol/meet2/README.md` and `descriptors.txt`; we identify the
/// device by `(vendor_id, product_id)` and address the XU by entity id.
pub(crate) const XU_ENTITY_ID: u8 = 2;

/// Minimum firmware version this build supports.
/// Updated once the first hardware verification run lands.
pub(crate) const MIN_FW: &str = "0.0.0";
