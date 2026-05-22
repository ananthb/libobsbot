# Contributing to libobsbot

Welcome. Before you open a PR, read this — the sourcing rule is the most
important policy in the project.

## The sourcing rule

OBSBOT's Terms of Use prohibit reverse engineering of their software. To keep
libobsbot legally defensible, contributions must be derived only from observed
USB wire behavior and public sources. The following inputs are off-limits when
working on this codebase:

- The contents of OBSBOT's `libdev.so`/`libdev.dylib`/`libdev.dll` — do not
  disassemble, decompile, dump strings, run `nm`/`readelf -a`/`objdump` for
  anything beyond confirming the file is what it claims to be, or look at the
  symbol table.
- Anything you obtained from OBSBOT under a sign-up / NDA agreement, including
  the official SDK toolkit they email after the application form. Even seeing
  it disqualifies you from contributing protocol code to this project.
- Decompiled output from any source (Ghidra, IDA, retdec, online services) of
  any OBSBOT binary.

The following inputs **are** permitted:

- The public C++ header (`sdk/v1.0.2/include/dev/dev.hpp` and friends) that
  OBSBOT ship with their sample code. The header is API shape only, no
  implementation.
- USB packet captures of the existing closed-source `libdev.so` talking to a
  real OBSBOT camera, captured with Wireshark + `usbmon` (or equivalent on
  other OSes). The capture is observed wire behavior, not source.
- Prior public work in other open-source projects (e.g.
  [samliddicott/meet4k](https://github.com/samliddicott/meet4k),
  [taxfromdk/obsbot_tiny_reversing](https://github.com/taxfromdk/obsbot_tiny_reversing)).
- OBSBOT's published documentation: OSC docs, manuals, public datasheets.
- Public Linux kernel patches involving OBSBOT cameras.

## The audit trail

Every selector value, payload byte layout, or protocol assumption in source
**must** point at a committed `.pcapng` under `doc/protocol/<model>/` and a
companion `.md` file that describes how the bytes were derived. The commit
that introduces the constant must reference the capture in its message:

```
meet2: add brightness selector

Captured 2026-05-22 with usbmon1 + Wireshark while sliding the brightness
control in OBSBOT's GUI through min/mid/max values.

Refs: doc/protocol/meet2/setBrightness.pcapng,
      doc/protocol/meet2/setBrightness.md
```

A PR that adds a magic constant without a capture cannot be merged.

## Style

- Rust edition 2021, MSRV 1.75. `cargo fmt --check` and `cargo clippy
  --workspace -- -D warnings` are CI gates.
- No `unsafe` outside the FFI crate's `extern "C"` boundary and `transport/usb.rs`.
- No `tokio` or other async runtimes in `libobsbot-core`. One long-lived thread
  per opened device is plenty.
- Comments only where the *why* is non-obvious (a workaround, an invariant, a
  hidden constraint). Don't narrate.

## Commits and PRs

- One logical change per PR.
- Conventional commit prefixes are fine but not required (`meet2:`, `transport:`,
  `ffi:`, `docs:`, `ci:`).
- Reference issues with `Closes #N` in the commit body where applicable.
- Run `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` locally before
  pushing.
