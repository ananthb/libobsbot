# setMediaMode.pcapng — XU mode-register: media mode

**Capture conditions**

- Date: 2026-05-23
- Camera: OBSBOT Meet 2, serial `RMOMWYI1141LCV`, firmware `4.4.6.1`,
  bus 3 device 15.
- Driver: `tools/meet2-exercise --set-media-mode 2` (value 2 =
  `MediaModeAutoFrame` in the SDK's `Device::MediaMode` enum).
- Capture filtered to `usb.device_address == 15`. 7.5 KB, 60 frames.

## Findings

| Frame | API call                              | Selector | Entity | wLen | First 3 bytes |
|------:|---------------------------------------|---------:|-------:|-----:|---------------|
| 58    | `cameraSetMediaModeU(MediaModeAutoFrame)` | `0x06`   | 2 (XU) |   60 | `00 01 02`    |

Trailing 57 bytes are zero padding.

Format matches `setWdr.md` exactly: `[control_id, 0x01, value, 0x00 × 57]`
on selector `0x06`. The control id for media mode is `0x00`. The value
byte matches the SDK enum value directly:

| `Device::MediaMode`     | wire value |
|-------------------------|-----------:|
| `MediaModeNormal`       | `0x00`     |
| `MediaModeBackground`   | `0x01`     |
| `MediaModeAutoFrame`    | `0x02`     |

`MediaModeNormal = 0` is also covered indirectly by `initial_apply.pcapng`
frame 676 (`00 01 00 ...`), which was the very first `setMediaMode` SET
observed.

## Code

`crates/libobsbot-core/src/devices/meet2.rs`:

```rust
pub(crate) const MODE_MEDIA_MODE: u8 = 0x00;
```

`Device::set_media_mode` writes
`mode_register_payload(MODE_MEDIA_MODE, encode_media_mode(mode))` to
`(XU_ENTITY_ID, XU_SEL_MODE_REGISTER)`.
