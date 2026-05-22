# meet2-exercise

Tiny C++ driver that calls one OBSBOT libdev.so API per invocation, so
a `dumpcap` on the matching `usbmon<bus>` interface sees exactly the
wire bytes for that one method — nothing else.

This sits outside the cargo workspace because it links the closed-source
`libdev.so` and is only used by maintainers when capturing pcaps for
`doc/protocol/<model>/`. See the [sourcing rule](../../doc/sourcing.html):
running `libdev.so` is permitted; decompiling it is not.

## Build

Needs the public OBSBOT SDK headers and `libdev.so` at runtime. You can
unpack them from any OBSBOT release tarball or grab a working copy from
[aaronsb/obsbot-camera-control](https://github.com/aaronsb/obsbot-camera-control)
under `sdk/v1.0.2/`.

```sh
OBSBOT_SDK=/path/to/sdk/v1.0.2
g++ -std=c++17 -O2 \
  -I$OBSBOT_SDK/include \
  -L$OBSBOT_SDK/lib -Wl,-rpath,$OBSBOT_SDK/lib \
  meet2_exercise.cpp -ldev -lpthread -o meet2-exercise
```

## Use

```
./meet2-exercise --<api> <value>
  --set-media-mode  <0|1|2>          (Normal / Background / AutoFrame)
  --set-wdr         <0|1>            (off / DOL2-to-1)
  --set-fov         <0|1|2>          (86 / 78 / 65)
  --set-face-ae     <0|1>
  --set-face-focus  <0|1>
```

## Capture workflow

The capture procedure documented in `doc/protocol/meet2/README.md` is:

```sh
# 1. Get usbmon access (one-time).
run0 modprobe usbmon
run0 setfacl -m u:$USER:r /dev/usbmon*

# 2. Identify the camera.
lsusb | grep -i obsbot
# e.g. Bus 003 Device 015: ID 3564:fefb OBSBOT Meet 2

# 3. Capture during one API call.
dumpcap -i usbmon3 -w /tmp/cap.pcapng -q &
sleep 1
./meet2-exercise --set-wdr 1
sleep 1
kill -INT %1

# 4. Filter to just the camera, save under doc/protocol/meet2/.
tshark -r /tmp/cap.pcapng \
  -Y 'usb.device_address == 15' \
  -w doc/protocol/meet2/setWdr.pcapng
```

The full unfiltered bus capture contains other USB traffic; the filtered
copy is what gets committed.
