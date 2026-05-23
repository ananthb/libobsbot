# macOS port plan (handoff doc)

This is a session handoff. The macOS USB transport is partially
scaffolded; the actual IOKit calls need to land on a machine that
can compile + test them against a real OBSBOT Meet 2. Point the next
Claude session at this file (`read doc/macos-port-plan.md and
continue`).

## What's already done

- Commit `98b7154` (`core: macos transport scaffold`) added:
  - `cfg(target_os = "macos")` deps in
    `crates/libobsbot-core/Cargo.toml`: `core-foundation 0.10`,
    `core-foundation-sys 0.8`, `io-kit-sys 0.4`, `mach2 0.4`, `libc`.
    No `nusb`. No async runtime.
  - `crates/libobsbot-core/src/transport/macos.rs` - stub
    `MacosTransport` that returns `Err(Unsupported)` from
    `open`, `uvc_set`, `uvc_get`; empty `enumerate()` function.
  - `crates/libobsbot-core/src/discovery.rs` - 3-way platform
    dispatch (`linux` / `macos` / `other`). `DeviceInfo` carries
    `registry_id: u64` on macOS so we can re-find a device across
    enumeration cycles.

Linux build untouched (47 unit + 1 integration + 5 ffi tests pass,
clippy clean).

## First task on the Mac

Confirm the scaffold compiles before changing anything else.

```bash
cd <repo>
git pull
cargo build           # should build clean
cargo test            # should pass (most tests are platform-agnostic
                      #  Transport mocks; nothing macOS-specific yet)
cargo clippy --workspace --all-targets -- -D warnings
```

If any of those fail, fix the build *before* writing IOKit code. The
likely failure modes are:

- `io-kit-sys` version drift (the published crate may have moved
  past 0.4; if so bump and adjust the imports the next step uses).
- `core-foundation` API churn between 0.9 and 0.10.
- `mach2` needing the `port` feature.

## Step 1 - enumerate USB devices via IOKit

Replace `transport::macos::enumerate()` (currently returns
`Vec::new()`) with a real IOKit walk:

```rust
pub(crate) fn enumerate() -> Vec<crate::discovery::DeviceInfo> {
    // 1. Build a matching dictionary for IOUSBDevice with
    //    idVendor = 0x3564.
    // 2. IOServiceGetMatchingServices(kIOMainPortDefault, dict, &iter)
    //    (kIOMasterPortDefault on older SDKs).
    // 3. For each io_service_t in the iterator:
    //    - read CFNumber properties: idVendor, idProduct, USB Serial
    //      Number, IORegistryEntryIDMatching
    //    - filter idVendor == 0x3564 and idProduct in our known set
    //      (currently just meet2::PRODUCT_ID_MEET2 = 0xfefb)
    //    - build a DeviceInfo with vendor_id, product_id,
    //      product_type, serial (empty if iSerial absent), and
    //      registry_id from IORegistryEntryGetRegistryEntryID.
    //    - IOObjectRelease the service.
    // 4. IOObjectRelease the iterator.
}
```

Useful crate references:

- `io_kit_sys::IOServiceMatching` (returns
  `*mut CFMutableDictionary`).
- `io_kit_sys::IOServiceGetMatchingServices`.
- `io_kit_sys::IORegistryEntryCreateCFProperty` (read a single
  property by name as a `CFTypeRef`).
- `io_kit_sys::IORegistryEntryGetRegistryEntryID` for the
  registry id.
- `core_foundation::number::CFNumber` /
  `core_foundation::string::CFString` for property values.
- The CF key names are `kUSBVendorID`, `kUSBProductID`,
  `kUSBSerialNumberString`. Use `CFString::from_static_string`.

Verify by running the existing `smoke` example:

```bash
cargo run --example smoke -p libobsbot-core
```

It will hit `Devices::open(info)` which calls
`MacosTransport::open` (still `Unsupported`), so it'll print an
error on `open` - but the list of detected cameras above that
should now include the Meet 2.

Commit when verified:
> `core(macos): enumerate Meet 2 via IOKit (no open/IO yet)`

## Step 2 - open the VideoControl interface

`MacosTransport::open(info)` needs to:

1. Re-find the device using `info.registry_id`:
   `IORegistryEntryIDMatching(info.registry_id)` returns a dict;
   feed it to `IOServiceGetMatchingService`.
2. Create an `IOUSBDeviceInterface**` via
   `IOCreatePlugInInterfaceForService(usbDevice,
   kIOUSBDeviceUserClientTypeID, kIOCFPlugInInterfaceID, &plugin,
   &score)` then `(*plugin)->QueryInterface(plugin,
   kIOUSBDeviceInterfaceID, &deviceInterface)`.
3. `(*deviceInterface)->USBDeviceOpen(deviceInterface)` to claim it.
4. Iterate interfaces with
   `(*deviceInterface)->CreateInterfaceIterator` using a request
   that matches `bInterfaceClass = 0x0e` (Video).
5. For each `io_service_t` interface, create an
   `IOUSBInterfaceInterface**` the same plugin way, then check
   `bInterfaceSubClass = 0x01` (VideoControl). That's the one we
   want. The Meet 2 has it as `bInterfaceNumber = 0`; we still pick
   by subclass for robustness.
6. `(*interfaceInterface)->USBInterfaceOpen(interfaceInterface)` -
   this is the call that conflicts with FaceTime / Zoom / OBS if
   they have the camera open.
7. Store the `IOUSBInterfaceInterface**` and the
   `IOUSBDeviceInterface**` in `MacosTransport`. The struct fields
   need to be wrapped in something `Send + Sync` since `Transport`
   requires it; the COM-style pointers are thread-safe per Apple's
   docs but the type system doesn't know - wrap in
   `Mutex<*mut IOUSBInterfaceInterface>` or document the
   safety contract and use raw pointers + `unsafe impl Send + Sync`.

The `IOUSB*Interface*` types aren't in `io-kit-sys`. They need
inline FFI declarations - look at
`<IOKit/usb/IOUSBLib.h>` (in the Xcode SDK) for the vtable
structure. Specifically you need:

- `IOUSBDeviceInterface` (`kIOUSBDeviceInterfaceID`):
  `USBDeviceOpen`, `USBDeviceClose`, `CreateInterfaceIterator`,
  `GetDeviceVendor`, `GetDeviceProduct`, ...
- `IOUSBInterfaceInterface` (`kIOUSBInterfaceInterfaceID`):
  `USBInterfaceOpen`, `USBInterfaceClose`, `ControlRequest`,
  `ControlRequestAsync`, `GetInterfaceNumber`, ...

`UUID` constants like `kIOUSBDeviceInterfaceID` need to be defined
as `CFUUIDGetConstantUUIDWithBytes(...)` literals. Apple ships these
in IOUSBLib.h - copy the bytes verbatim into the Rust source.

Commit when verified by running smoke and seeing
`Devices::open` succeed at construction (`learn_mac` will still
fail at this point; that's expected):
> `core(macos): open VideoControl interface via IOKit USB`

## Step 3 - ControlRequest for uvc_set / uvc_get

`MacosTransport::uvc_set(entity, selector, payload)` becomes:

```rust
let mut req: IOUSBDevRequest = unsafe { std::mem::zeroed() };
req.bmRequestType = 0x21;           // host -> device, class, interface
req.bRequest      = 0x01;           // SET_CUR
req.wValue        = (u16::from(selector) << 8) | 0;
req.wIndex        = (u16::from(entity_id) << 8) | u16::from(interface_number);
req.wLength       = payload.len() as u16;
req.pData         = payload.as_ptr() as *mut _;
// Call (*interface)->ControlRequest(interface, pipe_ref=0, &mut req);
```

Notes:

- `bRequest` for UVC GET maps from our `UvcGet` enum:
  `Cur=0x81, Min=0x82, Max=0x83, Res=0x84, Len=0x85, Info=0x86,
  Def=0x87`. The Linux side already does this; reuse the same
  conversion table from `crate::uvc::UvcGet`.
- `pipe_ref = 0` means the control pipe (endpoint 0).
- `IOUSBDevRequest`'s `wLenDone` returns the actual byte count - use
  that as the `Result<usize>` for `uvc_get`.
- `ControlRequest` returns an `IOReturn` (i32); 0 = `kIOReturnSuccess`.
  Map non-zero to `Error::Usb(format!("IOReturn 0x{ret:08x}"))`.

Once this lands, the smoke binary should run end-to-end on macOS the
same way it does on Linux: brightness reads as 100, pan/tilt centres
to 0/0, WDR/AI/face_focus toggles work, firmware/serial come back
from `learn_mac` + the RPC path.

Commit when verified:
> `core(macos): ControlRequest plumbing for uvc_set / uvc_get`

## Step 4 - cleanup

- Add macOS hot-plug. The Linux path polls
  `/sys/class/video4linux` every 2 s; on macOS the equivalent is
  `IOServiceAddMatchingNotification` with the same matching dict +
  a run-loop source. Easier interim: poll `enumerate()` the same
  way the Linux watcher does (the polling thread in `discovery.rs`
  already does this; it'll just work once `enumerate()` returns
  real entries).
- Update `doc/index.html` and `doc/hardware.html` to flip
  "macOS planned" to "macOS working".
- Add Garnix `darwin` build to `flake.nix` if it isn't already
  (the linux check matrix is the model).

## Critical constraints

1. **AVFoundation exclusivity.** When FaceTime / Zoom / OBS /
   Cheese / any AVCaptureSession has the Meet 2 open,
   `USBInterfaceOpen` will return `kIOReturnExclusiveAccess`
   (`0xe00002c5`). The OBSBOT vendor SDK works around this by
   hitting `CMIOObject` properties on the device, which
   AVFoundation forwards even while it owns the camera. That's a
   *separate* Transport impl, not this one. Document the
   limitation in the error message and link the limitation in
   `doc/hardware.html`.

2. **Send + Sync on COM pointers.** Apple's docs say the IOKit USB
   interface objects are safe to call from any thread once
   `*Open` has succeeded. The Rust type system can't see that.
   Either wrap the pointer in `Mutex` (simple, fine for the
   single-threaded use we have) or write a wrapper newtype with
   `unsafe impl Send + Sync` + a comment citing the Apple docs.
   Either choice is defensible; the wrapper-with-unsafe-impl
   choice is what `nusb` and `rusb` do internally.

3. **IOObject lifetime.** Every `io_service_t` /
   `io_iterator_t` from `IOServiceGetMatchingService(s)` needs
   `IOObjectRelease`. Every CF object pulled out of a property
   needs `CFRelease`. The Rust `core_foundation` crate has RAII
   wrappers that handle this; use them for CF objects. For IOKit
   `io_object_t` there's no Rust RAII wrapper in `io-kit-sys`,
   so wrap in a small `IOObject(io_object_t)` newtype with a
   `Drop` impl.

## Reference values

- USB vendor id: `0x3564` (Remo Tech Co., Ltd.)
- Meet 2 product id: `0xfefb`
- VideoControl interface number: 0 (per
  `doc/protocol/meet2/descriptors.txt`)
- XU entity id: 2
- bcdDevice: 5.10
- Mode-register selector: 0x06
- RPC channel selector: 0x02
- The full descriptor dump is at
  `doc/protocol/meet2/descriptors.txt`.

The XU GUID is `{9a1e7291-6843-4683-6d92-39bc7906ee49}` in normal
form. Raw bytes in the descriptor:
`91 72 1e 9a 43 68 83 46 6d 92 39 bc 79 06 ee 49`. macOS does NOT
match XUs by GUID for control transfers - we address by
`(entity_id, selector)` like Linux does.

## Verification recipe

After each step, run:

```bash
cargo build               # must be clean
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace    # the Transport mock tests stay green
                          # regardless of platform
cargo run --example smoke -p libobsbot-core
```

The smoke binary prints status as it goes. Compare against the
Linux output (in this file's commit history or in the most recent
smoke run captured in earlier commit messages):

```
opened OBSBOT Meet 2
  brightness = 100  (range 0..=100)
  contrast = 100  (range 0..=100)
  saturation = 100  (range 0..=100)
center pan/tilt... ok
set WDR off (XU selector 0x06)... ok
set AI mode off ... ok
set auto-framing Group ... ok
set audio AGC off ... ok
set face-focus off (XU RPC, synthesised frame)... ok
firmware from camera ... 4.4.6.1
serial from camera ... RMOMWYI1141LCV
  pan/tilt = (+0.000, +0.000)
  zoom = 0
  focus = 0
XU mode-register readbacks (after our SETs above):
  wdr           = Off
  face_ae       = false
  ai_mode       = None
status() snapshot... fw=4.4.6.1 sn=<your-meet-2-serial> b=100 c=100 s=100 ...
```

If you see this output on macOS, the port is functionally complete.

## Files to read first

In order:

1. `crates/libobsbot-core/src/transport/mod.rs` - the `Transport`
   trait the macOS impl needs to satisfy.
2. `crates/libobsbot-core/src/transport/usb.rs` (Linux's
   `UsbTransport`) - the working reference implementation. The
   macOS one should have the same public shape.
3. `crates/libobsbot-core/src/transport/uvcvideo.rs` - the actual
   ioctl building/decoding. Most of it (e.g. how `UvcGet` maps to
   bRequest) is platform-independent and should be lifted into a
   shared module rather than duplicated.
4. `crates/libobsbot-core/src/discovery.rs` - where the macOS
   dispatch lives.
5. `crates/libobsbot-core/src/transport/macos.rs` - the stub to
   replace.

## Don't go down these rabbit holes

- **AVCaptureDevice / CMIO.** Tempting because it works while
  another app has the camera open, but the OBSBOT-specific
  controls aren't exposed through `kCMIOObjectPropertyClass`
  unless OBSBOT publishes a CMIO plugin (they don't). Direct
  IOKit is the right path; the FaceTime conflict is a known
  limitation, not a problem to engineer around in v1.
- **libusb / rusb / nusb.** The user previously dropped nusb
  intentionally (commit `18f2071` - "drop nusb, enumerate via
  sysfs"). Don't reintroduce it.
- **Async.** This library is synchronous by design. Don't add
  `tokio` or `async-std` even though `IOServiceAddMatchingNotification`
  uses a run loop. Use a polling thread like Linux already does.
