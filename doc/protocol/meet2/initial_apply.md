# initial_apply.pcapng - first XU capture, exploratory

**Capture conditions**

- Date: 2026-05-23
- Camera: OBSBOT Meet 2, serial `RMOMWYI1141LCV`, firmware `4.4.6.1`,
  bus 3 device 32, USB ID `3564:fefb`, `bcdDevice = 5.10`.
- Capture interface: `usbmon3`.
- Driver: `aaronsb/obsbot-camera-control@HEAD` `obsbot-cli` (non-interactive
  mode, default config - no `~/.config/obsbot-control/settings.conf`).
- Tool versions: `tshark` / `dumpcap` 4.6.5, kernel `usbmon`.

The CLI applied a default configuration in one shot (mediaMode=Normal,
HDR=off, FOV=Wide, faceAE=off, faceFocus=off, zoom=1x, pan/tilt=0,0,
brightness=128, contrast=128, saturation=128, white-balance=auto). 1216
packets, 429 KB on disk. **No individual selector can be promoted to a
constant in `meet2.rs` from this capture alone** - per-method captures are
needed for the audit trail rule in `CONTRIBUTING.md`. This file documents
the high-level structural observations.

## Top-level finding

OBSBOT splits control across **two distinct surfaces**, not just the XU:

1. **Standard UVC controls** on the Camera Terminal (entity id 1) and
   Processing Unit (entity id 3). These are documented in UVC 1.5 §A.9.4–5,
   not OBSBOT-specific, and need no further per-method audit trail.
2. **OBSBOT vendor XU** on entity id 2 (GUID
   `{9a1e7291-6843-4683-6d92-39bc7906ee49}`). All proprietary behaviors live
   here.

This invalidates the v0.0.0 method skeleton in `crates/libobsbot-core/src/device.rs`
that routes every setter through `Transport::xu_set` - `brightness`,
`contrast`, `saturation`, `zoom`, `pan/tilt`, `focus`, and the WB-auto
toggle are standard UVC and should not touch the XU.

## Observed standard UVC traffic

Filter: `usb.bmRequestType == 0x21 && usb.setup.bRequest == 0x01`.

| Frame | wIndex | Entity | Selector                                        | wLen | Payload   | Meaning              |
|------:|-------:|-------:|-------------------------------------------------|-----:|-----------|----------------------|
| 694   | 0x0100 | CT (1) | 0x0b `CT_ZOOM_ABSOLUTE_CONTROL`                 |    2 | `00 00`   | zoom = 0             |
| 702   | 0x0100 | CT (1) | 0x0d `CT_PANTILT_ABSOLUTE_CONTROL`              |    8 | 8x `00`   | pan=0, tilt=0        |
| 706   | 0x0100 | CT (1) | 0x0d `CT_PANTILT_ABSOLUTE_CONTROL`              |    8 | 8x `00`   | repeat (libdev.so)   |
| 710   | 0x0300 | PU (3) | 0x02 `PU_BRIGHTNESS_CONTROL`                    |    2 | `64 00`   | brightness = 100     |
| 714   | 0x0300 | PU (3) | 0x03 `PU_CONTRAST_CONTROL`                      |    2 | `64 00`   | contrast = 100       |
| 718   | 0x0300 | PU (3) | 0x07 `PU_SATURATION_CONTROL`                    |    2 | `64 00`   | saturation = 100     |
| 722   | 0x0300 | PU (3) | 0x0b `PU_WHITE_BALANCE_TEMPERATURE_AUTO_CONTROL`|    1 | `01`      | WB auto = on         |

The CLI reported `brightness = 128` but wire bytes show 100. Inferred:
`libdev.so` normalises the user-facing value to the camera's reported
hardware range. Our SDK should expose the standard UVC range as-is (clients
can normalise on their side).

## Observed OBSBOT XU traffic

Filter: `usb.setup.wIndex == 0x0200`. Two selectors seen during this run:

### Selector 0x02 - RPC-framed channel

`SET_CUR` and `GET_CUR` always with `wLength = 60`. Payload has a recurring
header:

```
offset 0:     0xAA              magic
offset 1:     seq               low 10 bits forced to 0x1AA by libdev;
                                bits 5-6 toggle the inner CRC below
offset 2:     sub-seq           per-message in a session
offset 3:     0x00              reserved
offset 4-5:   0x0C 0x00         outer length (12, u16 LE)
offset 6-7:   outer CRC         CRC-16/USB over buf[0..outer_len], see
                                crc-investigation.md
offset 8:     0x0A              request direction marker
offset 9:     cmd_set
offset 10:    cmd_id
offset 11:    sub_cmd_id
offset 12-13: inner length      payload byte count, u16 LE
offset 14-15: inner CRC         CRC-16/USB over buf[12..16+inner_len]
                                when (seq & 0x60) != 0; zero otherwise
offset 16..   payload
padded to 60 bytes with 0x00
```

Frame 728 is the only `SET` on this selector during the apply phase; the
rest are device-info handshake (frames 190-274). Decoding individual
`(cmd_set, cmd_id)` pairs needs targeted captures.

### Selector 0x06 - single-byte mode register

Four consecutive `SET`s with payload `<value> 0x01 0x00×58` map 1:1 to the
CLI's four enum setters, in source order:

| Frame | First byte | CLI call                          |
|------:|-----------:|-----------------------------------|
| 676   | `0x00`     | `cameraSetMediaModeU(0)` (Normal) |
| 680   | `0x01`     | `cameraSetWdrR(1)` (HDR off)      |
| 684   | `0x04`     | `cameraSetFovU(4)` (Wide)         |
| 688   | `0x03`     | `cameraSetFaceAER(3)` (off)       |

The selector is the same for four different controls. The trailing `0x01`
is constant. **Inference, not confirmation:** the camera distinguishes
controls by `cmd_set`/`cmd_id` echoed through the preceding selector-0x02
RPC frame, not from selector 0x06 alone. Targeted single-control captures
will confirm or refute this.

## What goes into source from this capture

Nothing additional - descriptor-derived constants are already in
`meet2.rs`. Code routes for brightness/contrast/saturation/zoom/pan-tilt/
focus/wb-auto should be re-pointed at standard UVC selectors in a separate
change (no pcap audit trail required; UVC 1.5 §A.9.4–5 is the source).
XU constants stay pending per-method pcaps.

## What captures to plan next

1. Brightness sweep through `GET_MIN`, `GET_MAX`, `SET_CUR(min)`,
   `SET_CUR(mid)`, `SET_CUR(max)` - confirms the standard UVC PU path
   end-to-end and gives a golden-byte test fixture.
2. Single-control XU captures for `mediaMode`, `wdrR`, `fovU`, `faceAE`,
   `faceFocus`, `ai mode`, isolating each into its own pcap so the
   `<methodName>.pcapng` audit-trail rule applies cleanly.
3. Status read - capture an OBSBOT XU GET that returns device serial /
   firmware (`libdev.so` reports SN `RMOMWYI1141LCV` and FW `4.4.6.1`; the
   capture will show which `cmd_set`/`cmd_id` returns these).
