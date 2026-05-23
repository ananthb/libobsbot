# Audio AGC (microphone automatic gain control)

The Meet 2's USB Audio Class Feature Unit (UnitID 4 on interface 2)
only exposes Mute + Volume per the USB descriptor
(`bmaControls(0) = 0x0003` - bits for Mute, Volume; AGC bit 6 / value
`0x40` is NOT set). The SDK's `cameraSetAudioAGC` instead drives a
**video XU** control on selector `0x06`.

## Wire format

XU mode-register, control id `0x17`:

```
[0x17, 0x01, value, 0x00 × 57]   value: 0 = AGC off, 1 = AGC on
```

Same shape as `setFaceAE.md` / `setWdr.md` / `setMediaMode.md`.

## Source

Disassembled from `libdev.so` rather than captured from a wire trace:

- `Device::cameraSetAudioAGC(bool&)` at `0x62150` branches on
  `productType()`. The Tail-Air / Tiny-SE / etc. branches use the
  higher-level RPC `sendMsgAsync` with magic `0x6{6,8}000b00010000`.
  The catch-all branch (which the Meet 2 lands on) calls
  `DevicePrivate::uvcExtSet(selector=0x06, [0x17, 0x01, value, ...])`.
- `uvcExtSet` is a thin wrapper around `UvcProtocol::setData(buf,
  size=0x3c=60, selector, ...)`, identical to the
  `Transport::uvc_set(entity=2, selector=0x06, payload[60])` path our
  other mode-register controls use.

## Code

`Device::set_audio_agc(bool)` builds the payload via
`mode_register_payload(MODE_AUDIO_AGC, &[value])` and writes through
the XU. Verified end-to-end on the captured Meet 2: the camera
accepts both on and off without error.

## Getter status

The SDK's `cameraGetAudioAGC` doesn't take the same path - it goes
through `sendMsgSync` with magic `0x67000b00010000`, which is the
high-level RPC, not a UVC XU read. The XU status blob at selector
`0x06` `GET_CUR` (see [`statusBlob.md`](statusBlob.md)) doesn't
reflect AGC state, so we can't shortcut a getter through that path
either. Add an `audio_agc()` getter when an RPC GET capture lands.
