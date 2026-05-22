# OBSBOT Meet 2 protocol notes

This directory is the **audit trail** that backs every selector and byte
layout in `crates/libobsbot-core/src/devices/meet2.rs`. Every method that
makes it into source has a matching pair here:

- `<methodName>.pcapng` — raw USB capture, taken with Wireshark on `usbmon`
  while running OBSBOT's official `libdev.so` against a real Meet 2.
- `<methodName>.md` — human-readable notes derived from the pcap: XU GUID,
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

Device layout (also in `descriptors.txt`):

| Interface | Class           | Notes                                       |
|-----------|-----------------|---------------------------------------------|
| 0         | Video Control   | hosts the OBSBOT extension unit             |
| 1         | Video Streaming | MJPEG + YUY2 (UVC 1.0)                      |
| 2         | Audio Control   |                                             |
| 3         | Audio Streaming | multiple alt settings                       |

The raw GUID bytes in the descriptor are
`91 72 1e 9a 43 68 83 46 6d 92 39 bc 79 06 ee 49` — i.e. Microsoft
encoding, which is what `nusb` and the UVC class expect when matching the XU.

## Capture procedure

1. On Linux: `sudo modprobe usbmon && sudo setfacl -m u:$USER:r /dev/usbmon*`.
2. Plug in the Meet 2 and note bus/device with `lsusb | grep -i obsbot`.
3. Launch Wireshark on `usbmon<bus>`, capture filter
   `usb.device_address == <dev>`.
4. Run an OBSBOT GUI/CLI built against `libdev.so`.
5. Manipulate **only** the control under test — slide brightness to min,
   mid, max in sequence.
6. Stop the capture, save it here as `<methodName>.pcapng`.
7. In Wireshark, filter `usb.bmRequestType == 0x21 && usb.setup.bRequest == 0x01`
   for `SET_CUR` and `0xA1 && 0x81` for `GET_CUR`.
8. Record findings in `<methodName>.md`.

See `CONTRIBUTING.md` at the repo root for the sourcing rule.
