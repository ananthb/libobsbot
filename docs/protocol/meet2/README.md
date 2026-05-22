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

Filled in once the first capture lands.

| Field                  | Value           |
|------------------------|-----------------|
| USB vendor id          | _pending capture_ |
| USB product id (Meet 2)| _pending capture_ |
| XU GUID                | _pending capture_ |
| VideoControl interface | _pending capture_ |
| XU entity id           | _pending capture_ |

## Capture procedure

1. On Linux: `sudo modprobe usbmon && sudo setfacl -m u:$USER:r /dev/usbmon*`.
2. Plug in the Meet 2 and note bus/device with `lsusb | grep -i obsbot`.
3. Launch Wireshark on `usbmon<bus>`, capture filter
   `usb.device_address == <dev>`.
4. Run `aaronsb/obsbot-camera-control` against the official `libdev.so`.
5. Manipulate **only** the control under test — slide brightness to min,
   mid, max in sequence.
6. Stop the capture, save it here as `<methodName>.pcapng`.
7. In Wireshark, filter `usb.bmRequestType == 0x21 && usb.setup.bRequest == 0x01`
   for SET\_CUR and `0xA1 && 0x81` for GET\_CUR.
8. Record findings in `<methodName>.md`.

See `CONTRIBUTING.md` at the repo root for the clean-room rule.
