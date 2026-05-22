# setAutoFraming*.pcapng - XU mode-register: auto-framing sub-mode

**Capture conditions**

- Date: 2026-05-23
- Camera: OBSBOT Meet 2 on bus 3 device 15.
- Driver: `tools/meet2-exercise --set-auto-framing <group_single> <close_upper>`.
- Three captures (group, single+closeup, single+upperbody), filtered to
  `usb.device_address == 15`. ~6 KB each.

## Findings

Three `SET_CUR` frames, all on selector `0x06` entity 2:

| File                                 | API call                                                | First 4 bytes  |
|--------------------------------------|---------------------------------------------------------|----------------|
| `setAutoFramingGroup.pcapng`         | `cameraSetAutoFramingModeU(AutoFrmGroup, _)`            | `0d 02 00 00`  |
| `setAutoFramingSingleCloseUp.pcapng` | `cameraSetAutoFramingModeU(AutoFrmSingle, AutoFrmCloseUp)`   | `0d 02 01 00`  |
| `setAutoFramingSingleUpperBody.pcapng` | `cameraSetAutoFramingModeU(AutoFrmSingle, AutoFrmUpperBody)` | `0d 02 01 01`  |

Trailing 56 bytes are zero padding.

## Format

Selector-0x06 mode-register, exactly the layout documented in
`setAiMode.md`:

```
offset 0:    control_id    0x0d = auto-framing sub-mode
offset 1:    value_size    0x02 = 2-byte value field
offset 2:    group_single  0 = Group, 1 = Single
offset 3:    close_upper   0 = CloseUp, 1 = UpperBody  (ignored when group_single == 0)
offset 4..59: 0x00 padding
```

## Value mapping

`AutoFramingType` from `dev.hpp`:

| `AutoFramingType`    | wire byte | role            |
|----------------------|-----------|-----------------|
| `AutoFrmGroup`       | `0x00`    | group_single    |
| `AutoFrmSingle`      | `0x01`    | group_single    |
| `AutoFrmCloseUp`     | `0x00`    | close_upper     |
| `AutoFrmUpperBody`   | `0x01`    | close_upper     |

The SDK enum is overloaded: `AutoFrmGroup` and `AutoFrmCloseUp` share
value `0`, `AutoFrmSingle` and `AutoFrmUpperBody` share value `1`.
Our [`crate::AutoFramingMode`] flattens the three meaningful pairs
into named variants:

| `AutoFramingMode`     | wire value |
|-----------------------|------------|
| `Group`               | `[0, 0]`   |
| `SingleCloseUp`       | `[1, 0]`   |
| `SingleUpperBody`     | `[1, 1]`   |

## Code

```rust
pub(crate) const MODE_AUTO_FRAMING: u8 = 0x0d;
```

`Device::set_auto_framing` writes
`mode_register_payload(MODE_AUTO_FRAMING, &encode_auto_framing(mode))`
to `(XU_ENTITY_ID, XU_SEL_MODE_REGISTER)`.
