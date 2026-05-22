# OBSBOT Meet 2 protocol notes

This directory is the **audit trail** that backs every selector and byte
layout in `crates/libobsbot-core/src/devices/meet2.rs`. Every method that
makes it into source has a matching pair here:

- `<methodName>.pcapng` - raw USB capture, taken with Wireshark on `usbmon`
  while running OBSBOT's official `libdev.so` against a real Meet 2.
- `<methodName>.md` - human-readable notes derived from the pcap: XU GUID,
  control interface, entity id, selector, payload format for SET, payload
  format for GET, GET reply format, observed value range.

## Camera-level constants

Read from `lsusb -v -d 3564:fefb` against the unit used for capture.
The full descriptor dump is committed at `descriptors.txt` in this directory.

| Field                  | Value                                    |
|------------------------|------------------------------------------|
| USB vendor id          | `0x3564` (Remo Tech Co., Ltd.)           |
| USB product id (Meet 2)| `0xfefb`                                 |
| `bcdDevice`            | 5.10                                     |
| XU GUID                | `{9a1e7291-6843-4683-6d92-39bc7906ee49}` |
| VideoControl interface | `0`                                      |
| XU entity id           | `2`                                      |
| XU control count       | 7                                        |
| Serial string          | not exposed (`iSerial = 0`); reported by camera at runtime as `RMOMWYI1141LCV` |
| Firmware (observed)    | `4.4.6.1` on the capture unit            |

The raw GUID bytes in the descriptor are
`91 72 1e 9a 43 68 83 46 6d 92 39 bc 79 06 ee 49` - i.e. Microsoft
encoding, which is what the UVC class expects when matching the XU.

## Control-surface split (from `initial_apply.pcapng`)

The camera exposes two control surfaces, only one of which is OBSBOT-specific:

| Surface | Entity | What lives here                                                                                  |
|---------|-------:|--------------------------------------------------------------------------------------------------|
| Standard UVC `CameraTerminal` | 1 | zoom, pan/tilt, focus, exposure, roll - UVC 1.5 §A.9.4 selectors        |
| Standard UVC `ProcessingUnit` | 3 | brightness, contrast, saturation, hue, sharpness, gain, WB temp + auto, etc. - UVC 1.5 §A.9.5 |
| OBSBOT vendor extension       | 2 | mediaMode, HDR/WDR, FOV preset, face AE/focus, AI tracking, status - all proprietary           |

Standard UVC paths do not need per-method `.pcapng` files - UVC 1.5 §A.9
is the source. OBSBOT XU paths still require per-method captures per
`CONTRIBUTING.md`.

See `initial_apply.md` for the protocol observations behind this split.

## Capture procedure

The full per-OS recipe (what to install, exact commands for Linux /
macOS / Windows, how to filter and name the output) lives at
<https://ananthb.github.io/libobsbot/captures.html>.

For Linux specifically the short version is:

```sh
run0 modprobe usbmon
run0 setfacl -m u:$USER:r /dev/usbmon*
lsusb | grep -i obsbot       # note bus + device number
dumpcap -i usbmon<bus> -w /tmp/raw.pcapng -q &
sleep 1
# drive one method, e.g.
./tools/meet2-exercise/meet2-exercise --set-wdr 1
kill -INT %1 && wait
tshark -r /tmp/raw.pcapng -Y 'usb.device_address == <dev>' \
  -w doc/protocol/meet2/setWdr.pcapng
```

Record findings in `<methodName>.md` (one per pcap); see `setWdr.md`
for the worked example. The sourcing rule for what inputs are
admissible lives in `CONTRIBUTING.md` at the repo root.
