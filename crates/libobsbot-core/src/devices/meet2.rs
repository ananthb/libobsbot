// SPDX-License-Identifier: GPL-3.0-only
//! OBSBOT Meet 2 command table.
//!
//! Every constant in this module MUST be justified by a committed pcap under
//! `docs/protocol/meet2/`. See `CONTRIBUTING.md` for the clean-room rule.
//!
//! v0.0.0 contains placeholders only — no real selectors. The capture phase
//! starts at M3.

/// OBSBOT USB vendor id (placeholder; confirm via `lsusb` in M1).
pub(crate) const VENDOR_ID: u16 = 0x3564;

/// OBSBOT Meet 2 USB product id (placeholder; confirm in M1).
pub(crate) const PRODUCT_ID_MEET2: u16 = 0x0000;

/// Minimum firmware version this build supports.
/// Updated once the first hardware verification run lands.
pub(crate) const MIN_FW: &str = "0.0.0";
