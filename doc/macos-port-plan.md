# macOS transport design notes

This document records the decisions behind
`crates/libobsbot-core/src/transport/macos.rs` so the next time
someone reaches for IOKit they don't repeat the dead-ends.

## What ships

- Enumeration via `IOServiceGetMatchingServices(IOUSBDevice)`,
  filtered client-side by `idVendor` + `idProduct`. We do not put
  `idVendor` in the matching dictionary because on Apple Silicon the
  legacy `IOUSBDevice` class name is translated to
  `IOUSBHostDevice` via class inheritance and property-level filters
  don't apply across that translation. `libusb` makes the same
  client-side filter choice.
- A registry-entry id stored on `DeviceInfo` so `MacosTransport::open`
  can re-find the same device across enumeration cycles
  (`IORegistryEntryIDMatching`).
- Control transfers via `IOUSBDeviceInterface::DeviceRequest` on the
  device's default control pipe (endpoint 0). The UVC
  `(entity, selector)` pair is encoded into `wValue` / `wIndex`
  exactly as on Linux; the `bmRequestType` is `0x21` (host->device,
  class-specific, recipient=interface) for SETs and `0xa1` for GETs.
- `wIndex = (entity << 8) | bInterfaceNumber`, with
  `bInterfaceNumber` read from the `VideoControl` interface's
  `io_service_t` (no `USBInterfaceOpen` required - we read the
  registry-entry CF property directly).
- No `USBDeviceOpen`. Per Apple's documented contract for
  `DeviceRequestTO` ("the device does not have to be open to use this
  function"), class-specific transfers go through without an
  exclusive device claim, which means we don't conflict with any
  other client.

## The fight we didn't win: `UVCAssistant`

On macOS Big Sur and above (and especially on Apple Silicon),
`/System/Library/Frameworks/CoreMediaIO.framework/.../UVCAssistant.systemextension`
attaches to every UVC device at plug-in time and holds an exclusive
claim on the `VideoControl` and `VideoStreaming` interfaces.
`ioreg -p IOUSB -l` shows it as
`"UsbExclusiveOwner" = "pid <N>, UVCAssistant"`.

The original handoff plan called for
`IOUSBInterfaceInterface::USBInterfaceOpen` followed by
`ControlRequest` on the interface. Both `USBInterfaceOpen` and
`USBInterfaceOpenSeize` fail against `UVCAssistant` with
`kIOReturnExclusiveAccess` (`0xe000_02c5`) - the system extension
declines to yield even on Seize, and there is no
user-space-accessible knob to override that. This is the path the
handoff plan worried about needing a `CMIOObject`-based workaround
for; in practice `DeviceRequest` at the device level skips the fight
entirely because endpoint 0 isn't anyone's exclusive resource.

The cost is small: we lose the ability to read pipe descriptors or
do bulk-endpoint transfers, neither of which this SDK does. If a
future scope expansion needs streaming or bulk transfers, those will
still have to coexist with `UVCAssistant` - probably via
`AVCaptureSession` rather than raw IOKit.

## Other paths considered

- **`USBInterfaceOpenSeize` via `kIOUSBInterfaceInterfaceID183`.**
  Tried, fails with `kIOReturnExclusiveAccess` against
  `UVCAssistant`. The vtable definition is still in git history if a
  future need arises but isn't worth carrying.
- **`CMIOObject` properties.** Apple exposes standard UVC controls
  (PU brightness/contrast/saturation/WB, CT pan/tilt/zoom/focus) via
  `CMIOObjectGetPropertyData` on the device. OBSBOT's vendor XU
  controls are not exposed and would need a custom CMIO plugin from
  OBSBOT to be reachable. `DeviceRequest` is strictly more capable
  for our use case.
- **`nusb`** / **`rusb`** / **`libusb`** crates. Dropped by an earlier
  commit (`18f2071`) on Linux; reintroducing them on macOS would
  pull in a transitive dependency that we don't need. Inline FFI to
  the IOKit COM ABI is well under a thousand lines and stays under
  our direct control.
- **Async runtime.** This library is synchronous by design. The hot-
  plug watcher polls every 2 s; switching to
  `IOServiceAddMatchingNotification` would need a run-loop source
  thread but isn't worth it for the latency win at v1.

## Tested behaviour

`cargo run --example smoke -p libobsbot-core` on macOS 15 against a
Meet 2 with FaceTime/Photo Booth idle:

```
opened OBSBOT Meet 2
  brightness = 100  (range 0..=100)
  ...
firmware from camera (XU RPC, synthesised frame)... 4.4.6.1
serial from camera (XU RPC, synthesised frame)... RMOMWYI1141LCV
```

All XU controls (WDR / face AE / AI mode / auto-framing / audio AGC /
face focus) and standard UVC controls (brightness/contrast/saturation,
pan/tilt, zoom, focus) round-trip cleanly. The same smoke binary
works when Photo Booth is also showing the camera preview - the
exclusivity fight was never necessary.

## Reference values

- USB vendor id: `0x3564` (Remo Tech Co., Ltd.)
- Meet 2 product id: `0xfefb`
- VideoControl interface number: 0 (per
  `doc/protocol/meet2/descriptors.txt`)
- XU entity id: 2
- Mode-register selector: `0x06`
- RPC channel selector: `0x02`

Property names on the `IOUSBDevice` registry entry: `idVendor`,
`idProduct`, `USB Serial Number`, `bInterfaceNumber` (the last one on
the `IOUSBHostInterface` child entry, not the device).

UUIDs we use (constants in `transport/macos.rs`):

- `kIOUSBDeviceUserClientTypeID` =
  `9dc7b780-9ec0-11d4-a54f-000a27052861`
- `kIOCFPlugInInterfaceID` =
  `c244e858-109c-11d4-91d4-0050e4c6426f`
- `kIOUSBDeviceInterfaceID` (v100) =
  `5c8187d0-9ef3-11d4-8b45-000a27052861`

The v100 device-interface UUID is sufficient. Apple has shipped
v182/v187/v197/v245/v300/v320/v400/v500/v650 supersets; every later
version layouts its new slots after the v100 ones, so a v100 vtable
declaration is still ABI-correct on modern macOS.
