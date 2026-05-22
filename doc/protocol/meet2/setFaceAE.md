# setFaceAE.pcapng — XU mode-register: face-based auto-exposure

**Capture conditions**

- Date: 2026-05-23
- Camera: OBSBOT Meet 2 on bus 3 device 15.
- Driver: `tools/meet2-exercise --set-face-ae 1`.
- Filtered to `usb.device_address == 15`. 7 KB, 54 frames.

## Findings

| Frame | API call                | Selector | Entity | wLen | First 3 bytes |
|------:|-------------------------|---------:|-------:|-----:|---------------|
| 52    | `cameraSetFaceAER(1)`   | `0x06`   | 2 (XU) |   60 | `03 01 01`    |

Trailing 57 bytes zero. Format `[control_id, 0x01, value, 0x00 × 57]`.
Control id for face-AE is `0x03`. Value byte: `0x00` = off, `0x01` = on.

`face_ae = 0` is covered indirectly by `initial_apply.pcapng` frame 688
(`03 01 00 ...`).

## Code

```rust
pub(crate) const MODE_FACE_AE: u8 = 0x03;
```

`Device::set_face_ae` writes
`mode_register_payload(MODE_FACE_AE, u8::from(on))` to the mode-register
selector.
