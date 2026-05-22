# setWdr.pcapng — XU mode-register: WDR / HDR

**Capture conditions**

- Date: 2026-05-23
- Camera: OBSBOT Meet 2, serial `RMOMWYI1141LCV`, firmware `4.4.6.1`,
  bus 3 device 15, USB ID `3564:fefb`.
- Capture interface: `usbmon3`, then filtered to `usb.device_address == 15`
  with `tshark -Y ...` to drop unrelated bus 3 traffic. 102 frames, 10 KB.
- Driver: `aaronsb/obsbot-camera-control@HEAD` `obsbot-cli --interactive`.
  Stdin: `h` (enable HDR) → `H` (disable HDR) → `q`.
  CLI debug printed `cameraSetWdrR ... successfully` twice, once per toggle.

The capture also covers the libdev.so handshake (frames 30, 38, 46, 54 are
selector-0x02 RPC; not the subject of this method note — see `initial_apply.md`
for that surface).

## Findings

Two `SET_CUR` frames bracket the HDR-on then HDR-off toggles:

| Frame | API call               | Selector | Entity | wLen | First 6 bytes of payload          |
|------:|------------------------|---------:|-------:|-----:|-----------------------------------|
| 70    | `cameraSetWdrR(on)`    | `0x06`   | 2 (XU) |   60 | `01 01 01 00 00 00`               |
| 82    | `cameraSetWdrR(off)`   | `0x06`   | 2 (XU) |   60 | `01 01 00 00 00 00`               |

Remaining 54 bytes are `0x00` padding.

The bytes pin the format for selector 0x06 (the OBSBOT XU "mode register"):

```
offset 0:    control id      0x01 = WDR
offset 1:    request flag    0x01 = SET (constant in observed writes)
offset 2:    value           0x01 = on  (DOL2-to-1)
                             0x00 = off (no HDR)
offset 3..59 padding (0x00)
```

The `01` at offset 1 is the same constant byte that appears in every
selector-0x06 SET observed in `initial_apply.pcapng` (mediaMode, FOV,
faceAE all carry it). Until a per-control capture exists for the other
three, the meaning is recorded as "request flag" — likely a "set, ack"
indicator.

## Code

`crates/libobsbot-core/src/devices/meet2.rs` carries:

```rust
pub(crate) const XU_SEL_MODE_REGISTER: u8 = 0x06;
pub(crate) const MODE_WDR: u8 = 0x01;
```

and `Device::set_wdr` writes
`[MODE_WDR, 0x01, value_byte, 0x00 × 57]` to
`(entity = XU_ENTITY_ID, selector = XU_SEL_MODE_REGISTER)`.

## What captures to plan next

Selector 0x06 multiplexes at least four controls (mediaMode, WDR, FOV,
faceAE). Per-control captures will confirm:

- `setMediaMode.pcapng` — control id `0x00` expected (`MediaModeNormal`
  set wrote `00 01 00` in `initial_apply.pcapng` — matches the
  `<value>` hypothesis only if `Normal = 0`).
- `setFov.pcapng` — control id ≠ value; the wire `04 01 00` in
  `initial_apply` for `cameraSetFovU(4)` (Wide) means the value lives in
  the first byte (and control id is implicit), or the format differs
  per control. A targeted single-FOV capture will distinguish.
- `setFaceAE.pcapng` — same question.

Until those land, only `MODE_WDR` is promoted into source.
