# setFaceFocus.pcapng — face-based auto-focus uses the RPC channel, not the mode-register

**Capture conditions**

- Date: 2026-05-23
- Camera: OBSBOT Meet 2 on bus 3 device 15.
- Driver: `tools/meet2-exercise --set-face-focus 1`.
- Filtered to `usb.device_address == 15`. 7.5 KB, 60 frames.

## Finding

Unlike `mediaMode`, `wdr`, `fov`, and `faceAE`, **face-focus does not
use the XU mode-register on selector `0x06`**. Instead `libdev.so`
sends an RPC-framed command on selector `0x02`:

| Frame | Selector | Entity | wLen | First 16 bytes of payload                                       |
|------:|---------:|-------:|-----:|-----------------------------------------------------------------|
| 58    | `0x02`   | 2 (XU) |   60 | `aa 25 04 00 0c 00 d8 c6 0a 02 02 36 04 00 bf fb`                |

Decoded against the recurring selector-0x02 header (`setWdr.md` /
`initial_apply.md`):

```
aa            0xAA magic
25            seq = 0x25
04            sub-seq = 0x04
00            reserved
0c 00         inner-payload length (12 LE)
d8 c6         CRC16 over … something we haven't pinned yet
0a            second magic
02            cmd_set
02            cmd_id
36 04 00      sub-command header
bf fb         variable bytes (probably the face-focus state)
01 00 00 …    trailing fixed bytes seen in other selector-0x02 calls
```

The same `(cmd_set=0x02, cmd_id=0x02, sub-header=36 04 00)` triple also
appears in `initial_apply.pcapng` frame 728 (`bf 07 00 ...`) when the
default config applied face-focus = off. Two values give two data points
but the bit-level mapping from "on/off" to `bf fb` vs `bf 07` is not yet
pinned — decoding it is part of the selector-0x02 RPC framing work in
task #9.

## Code

`Device::set_face_focus` remains a placeholder (`uvc_set` on selector
`0`) until the RPC framing decode lands. Comment in `device.rs` flags
this.

## Next captures

To distinguish "bit position of face-focus value" from "session-counter
noise" we'd need:

- `--set-face-focus 0` followed by `--set-face-focus 1` in the same
  run with the seq counter visible.
- Pair these with `initial_apply.pcapng` frame 728 (face-focus=off).
