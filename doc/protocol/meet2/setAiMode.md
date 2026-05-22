# setAiMode.pcapng - XU mode-register: AI master mode

**Capture conditions**

- Date: 2026-05-23
- Camera: OBSBOT Meet 2 on bus 3 device 15.
- Driver: `aaronsb/obsbot-camera-control` `obsbot-cli --interactive`,
  stdin `i 2` (enable Single Human tracking) then `I` (disable AI)
  then `q`.
- Filtered to `usb.device_address == 15`. ~70 frames, 8 KB.

## Finding

Two `SET_CUR` frames bracket the AI-on and AI-off toggles. Both ride
selector `0x06` on entity 2 (the XU mode register), exactly like the
WDR / FOV / faceAE / mediaMode controls but with a different value
encoding:

| Frame | API call                                  | Selector | Entity | wLen | First 4 bytes        |
|------:|-------------------------------------------|---------:|-------:|-----:|----------------------|
| 56    | `cameraSetAiModeU(AiWorkModeHuman = 2)`   | `0x06`   | 2 (XU) |   60 | `16 02 02 00`        |
| 64    | `cameraSetAiModeU(AiWorkModeNone = 0)`    | `0x06`   | 2 (XU) |   60 | `16 02 00 00`        |

Trailing 56 bytes are zero padding.

## Format generalisation

The byte at offset 1 of every selector-0x06 SET is **the value length
in bytes**, not the constant `0x01` we initially documented in
`setWdr.md`. The 1-byte controls (WDR / FOV / faceAE / mediaMode) all
have offset 1 = `0x01`; AI mode has offset 1 = `0x02` because the
value is a u16 LE at offsets 2-3.

Updated wire layout for selector `0x06`:

```
offset 0:    control_id     0x16 = AI master mode
offset 1:    value_size     bytes occupied by the value field
offset 2..N: value          little-endian, N = 2 + value_size
offset N..59: 0x00 padding
```

The helper `crates/libobsbot-core/src/devices/meet2.rs::mode_register_payload`
takes a value byte slice rather than a single byte to support this.

## Value mapping

`AiWorkModeType` from the SDK public header:

| `AiWorkModeType`          | wire value (u16 LE) | our `AiMode` |
|---------------------------|---------------------|--------------|
| `AiWorkModeNone`          | `0x0000`            | `AiMode::None`       |
| `AiWorkModeGroup`         | `0x0001`            | `AiMode::Group`      |
| `AiWorkModeHuman`         | `0x0002`            | `AiMode::Human`      |
| `AiWorkModeHand`          | `0x0003`            | `AiMode::Hand`       |
| `AiWorkModeWhiteBoard`    | `0x0004`            | `AiMode::WhiteBoard` |
| `AiWorkModeDesk`          | `0x0005`            | `AiMode::Desk`       |
| `AiWorkModeSwitching` / `AiWorkModeButt` | n/a  | not exposed (status / sentinel) |

## Code

```rust
pub(crate) const MODE_AI_MODE: u8 = 0x16;
```

`Device::set_ai_mode` writes
`mode_register_payload(MODE_AI_MODE, &(value as u16).to_le_bytes())` to
`(XU_ENTITY_ID, XU_SEL_MODE_REGISTER)`.
