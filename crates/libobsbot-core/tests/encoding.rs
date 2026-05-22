// SPDX-License-Identifier: GPL-3.0-only
//! Encoding tests for per-model command tables.
//!
//! v0.0.0 has no real selectors yet, so this file only proves the test
//! harness is wired up. Real golden-byte tests land alongside each method
//! starting at M3 - see `doc/protocol/meet2/<method>.md` for the expected
//! wire bytes.

#[test]
fn placeholder_until_capture_phase() {
    // intentionally trivial - replaced when M3 introduces the first
    // selector + encoding constant from a committed pcap.
}
