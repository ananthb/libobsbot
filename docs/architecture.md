# libobsbot architecture

## Overview

libobsbot is a Rust workspace with two member crates: `libobsbot-core` (pure
Rust library) and `libobsbot-ffi` (C ABI cdylib + staticlib generated from
`libobsbot-core`). The core crate is the only crate that touches USB.

## Layers

```
+---------------------------------------------------------------+
|  public Rust API (Devices, Device, Event, types)              |  src/lib.rs + src/device.rs + src/discovery.rs
+---------------------------------------------------------------+
|  per-model command table (selector, encode, decode triples)   |  src/devices/meet2.rs
+---------------------------------------------------------------+
|  Transport trait { xu_set, xu_get }                           |  src/transport/mod.rs
+---------------------------------------------------------------+
|  nusb UVC class-specific control transfers                    |  src/transport/usb.rs
+---------------------------------------------------------------+
|  nusb -> usbfs (Linux), IOKit (macOS), WinUSB (Windows)       |
+---------------------------------------------------------------+
```

The narrowest layer — `Transport` — is the boundary that mocks plug into for
encoding tests. Every selector and byte-layout constant in the command table
must be justified by a committed pcap; see `CONTRIBUTING.md`.

## Threading

- The hot-plug watcher runs on one `std::thread` spawned by `Devices::new`,
  pumping events into a `crossbeam_channel`.
- Each opened `Device` owns one status-poller thread that issues a `xu_get`
  for the status selector on a 2.5 s interval (slow mode) or 25 ms interval
  (fast mode), again pumping into the same channel.
- All public setter methods are synchronous. Getter methods that map to
  `xu_get` are also synchronous; the SDK's `Async` getter mode is not
  exposed in v1.

No `tokio` / async runtime. One long-lived thread per device is enough.

## Where to expect change

- **`src/devices/meet2.rs`** grows the entire `(selector, encode, decode)`
  table over M3 - M6. Every entry references a pcap.
- **`src/transport/usb.rs`** picks up the XU GUID match, entity-id discovery,
  and the actual `control_in`/`control_out` calls in M2/M3.
- **`src/discovery.rs`** picks up real `nusb::list_devices` + hot-plug in M1.

## What is intentionally out of scope (v1)

- Models other than the Meet 2.
- Firmware update, file transfer, recording controls, exposure modes beyond
  brightness/contrast/saturation, gimbal-preset CRUD, AI gesture, Bluetooth,
  WiFi, NDI/RTSP. Some of these use bulk endpoints rather than XU and would
  require an entirely separate transport layer (`RxDataCallback` shape in the
  SDK header). If v2 needs them, the `Transport` trait will grow.
