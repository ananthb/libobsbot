# libobsbot

A cross-platform Rust SDK for [OBSBOT](https://www.obsbot.com)
cameras. Implements the OBSBOT camera-control wire protocol from observed USB
traffic and ships a stable C ABI so the same library can drive PTZ, image
controls, white balance, HDR, FOV, face AE/focus, AI auto-framing, and audio
controls from any GPL-compatible application.

**Status:** v0.0.0 - pre-release. The Meet 2 protocol is decoded -
including the selector-0x02 RPC CRC - and the synchronous control
surface works end-to-end against real hardware. A per-device status
poller and a hot-plug watcher feed the same event channel; the C ABI
mirrors the Rust surface. Full docs at
<https://ananthb.github.io/libobsbot/>.

> **Have an OBSBOT camera we don't yet support?** The fastest way to
> add it is a USB packet capture. See the
> [hardware-support matrix](https://ananthb.github.io/libobsbot/hardware.html)
> and [open a new-hardware issue](https://github.com/ananthb/libobsbot/issues/new?template=new-hardware.yml)
> with a `.pcapng` attached - we'll do the protocol-decode work from
> there.

## Platforms

| OS | Notes |
|----|-------|
| Linux | First-class. Goal: work while the camera is also open in Zoom/OBS/Cheese - route through the `uvcvideo` driver rather than detaching it. |
| macOS | Supported. Works while other apps (FaceTime, Zoom, OBS, Photo Booth, …) have the camera open. |
| Windows | The camera must be bound to WinUSB via [Zadig](https://zadig.akeo.ie). Binding WinUSB removes the camera from `usbvideo.sys`, so it becomes unusable in Zoom/OBS until the driver is restored. |

## License

GPL-3.0-only. See `LICENSE`. Note that linking to `liblibobsbot` from a
non-GPL-compatible application is incompatible with this license.
