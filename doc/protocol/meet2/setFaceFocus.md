# setFaceFocus.pcapng - face-based auto-focus uses the RPC channel, not the mode-register

**Capture conditions**

- Date: 2026-05-23
- Camera: OBSBOT Meet 2 on bus 3 device 15.
- Driver: `tools/meet2-exercise --set-face-focus 1`.
- Filtered to `usb.device_address == 15`. 7.5 KB, 60 frames.

## Finding

Unlike `mediaMode`, `wdr`, `fov`, and `faceAE`, **face-focus does not
use the XU mode-register on selector `0x06`**. Instead `libdev.so`
sends an RPC-framed command on selector `0x02`. Two captures, one per
value:

- `setFaceFocusOn.pcapng` frame 52, value `01 00 00 00` at offset 16
- `setFaceFocusOff.pcapng` frame 52, value `00 00 00 00`

Common frame (with the value bytes underlined):

```
aa 25 04 00 0c 00 d8 c6 0a 02 02 36 04 00 bf fb 01 00 00 00 ...
                              ^^^^^                       ^^^^^^^^^^^
                              outer CRC                   payload value
```

## Decoded layout

Mapping the bytes to the selector-0x02 framing recovered from
`libdev.so` (see [`crc-investigation.md`](crc-investigation.md)):

```
aa            magic
25            seq        bit 5 set, so the inner CRC at [14,15] applies
04            sub-seq
00            reserved
0c 00         outer length = 12 (u16 LE)
d8 c6         outer CRC: CRC-16/USB over buf[0..12] with [6,7] zeroed
0a            request direction marker
02            cmd_set
02            cmd_id
36            sub-cmd-id
04 00         inner length = 4 (u16 LE)
bf fb         inner CRC: CRC-16/USB over buf[12..20] with [14,15] zeroed
              (varies between on/off because the payload at [16..20] differs)
01 00 00 00   payload: 1 = on, 0 = off (low byte is the boolean; rest reserved)
[..pad..]     bytes 20..26 zero, MAC tail at 26..32, sentinel 01 01 at 32-33
```

## Code

`Device::set_face_focus` builds this frame at runtime via
`meet2::build_rpc_frame(seq=0x25, sub_seq=0x04, cmd_set=0x02,
cmd_id=0x02, sub_cmd_id=0x36, payload=[on, 0, 0, 0], tail_offset=26,
tail=[..mac.., 0x01, 0x01])`, computing both CRCs from the device's
MAC learned at open time. Verified end-to-end against the live camera.
