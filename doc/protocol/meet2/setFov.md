# setFov.pcapng - XU mode-register: FOV preset

**Capture conditions**

- Date: 2026-05-23
- Camera: OBSBOT Meet 2 on bus 3 device 15.
- Driver: `tools/meet2-exercise --set-fov 1` (`FovType78` = medium / 78°).
- Filtered to `usb.device_address == 15`. 7 KB, 54 frames.

## Findings

| Frame | API call                       | Selector | Entity | wLen | First 3 bytes |
|------:|--------------------------------|---------:|-------:|-----:|---------------|
| 52    | `cameraSetFovU(FovType78)`     | `0x06`   | 2 (XU) |   60 | `04 01 01`    |

Trailing 57 bytes zero. Format `[control_id, 0x01, value, 0x00 × 57]`.
Control id for FOV is `0x04`. Value byte matches the SDK enum:

| `Device::FovType` | wire value |
|-------------------|-----------:|
| `FovType86` (Wide / 86°)   | `0x00` |
| `FovType78` (Medium / 78°) | `0x01` |
| `FovType65` (Narrow / 65°) | `0x02` |

`FovType86 = 0` is also covered indirectly by `initial_apply.pcapng`
frame 684 (`04 01 00 ...`).

## Code

```rust
pub(crate) const MODE_FOV: u8 = 0x04;
```

`Device::set_fov` writes
`mode_register_payload(MODE_FOV, encode_fov(fov))` to the mode-register
selector.
