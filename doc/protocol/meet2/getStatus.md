# getStatus.pcapng - selector 0x02 RPC framing, with the serial / firmware decode

**Capture conditions**

- Date: 2026-05-23
- Camera: OBSBOT Meet 2, serial `RMOMWYI1141LCV`, firmware `4.4.6.1`,
  bus 3 device 15.
- Driver: `aaronsb/obsbot-camera-control` `obsbot-cli --interactive`,
  stdin `0` (Get Camera Status) then `q`.
- Captured on `usbmon3`, filtered to `usb.device_address == 15`. ~60
  frames, 7 KB.

This file pins down the selector-0x02 RPC framing using the device
handshake (frames 20-49) and identifies where the serial and firmware
strings live in the reply bytes.

## Framing

Every selector-0x02 SET and GET on entity 2 carries a fixed 60-byte
payload (zero-padded). Layout, with the direction marker bytes 8-9
flipped between request and reply:

```
offset 0:    0xAA               magic
offset 1:    seq                fixed per session (0x01 for requests,
                                0x29 for replies in this capture)
offset 2:    sub-seq            increments per (request, reply) pair
offset 3:    0x00               reserved
offset 4-5:  0x0C 0x00          unknown small constant (12 LE);
                                doesn't track payload length
offset 6-7:  CRC                varies per frame, algorithm not yet
                                cracked - same bytes repeat across
                                identical content + sub-seq combos
                                so it's a real checksum, not a tag
offset 8:    direction byte 0   0x0A in requests, 0x0D in replies
offset 9:    direction byte 1   0x0D in requests, 0x0A in replies
                                (cmd_set in this capture is 0x0D;
                                whether the direction byte 0x0A is a
                                second marker or part of a swapped
                                little-endian pair is open)
offset 10:   cmd_id             function selector within cmd_set
offset 11:   sub-cmd-id         sub-function under cmd_id
offset 12-13:                   payload length (u16 LE) of data at
                                offset 16+
offset 14-15:                   two bytes that change per reply -
                                possibly a payload-content checksum
                                or a status / type tag. Unknown.
offset 16..  payload data       interpretation depends on
                                (cmd_id, sub-cmd-id)
padded to 60 bytes with 0x00.
```

## Handshake exchanges (frames 20-49)

The CLI does 4 RPC round-trips while opening the device. Each pairs
one SET (host -> camera) with one GET (camera -> host) on selector
0x02. Sub-seq increments 0-3 across the pair.

### Pair 0 - frames 20 + 25, `(cmd_id, sub-cmd-id) = (0x08, 0x18)`

Reply (frame 25):

```
aa 29 00 00 0c 00 a0 49 0d 0a 08 18 18 00 a1 1c
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
00 ad b6 1b 98 dc 8d 00 00 00 00 ...
```

- length = `18 00` = 24 bytes of data at offset 16
- bytes 32-37 = `ad b6 1b 98 dc 8d` = the "MAC tail" seen in the
  libdev.so debug log (`device info is: ... 000000...ADB61B98DC8D ...`)
- bytes 16-31 are all zero - the camera reports the leading zeros of
  a 48-hex-char identifier whose tail is the MAC

This is a "device hash" / extended ID read.

### Pair 1 - frames 28 + 33, `(cmd_id, sub-cmd-id) = (0x08, 0x04)`

Reply (frame 33):

```
aa 29 01 00 0c 00 f0 45 0d 0a 08 04 08 00 2d aa
01 06 04 04 00 00 ...
```

- length = `08 00` = 8 bytes of data at offset 16
- bytes 16-19 = `01 06 04 04` - **firmware version**, read byte 3
  first: `4.4.6.1`

This is the firmware-version read.

### Pair 2 - frames 36 + 41, `(cmd_id, sub-cmd-id) = (0x48, 0x19)`

Reply (frame 41):

```
aa 29 02 00 0c 00 f1 83 0d 0a 48 19 22 00 eb f0
0a 00 0a 02 01 06 04 04 00 00 ...
```

- length = `22 00` = 34 bytes of data at offset 16
- bytes 20-23 = `01 06 04 04` - **same firmware version, embedded**
  inside a larger 34-byte info blob (full meaning not yet decoded)

### Pair 3 - frames 44 + 49, `(cmd_id, sub-cmd-id) = (0xC8, 0x18)`

Reply (frame 49):

```
aa 29 03 00 0c 00 00 46 0d 0a c8 18 0e 00 22 38
52 4d 4f 4d 57 59 49 31 31 34 31 4c 43 56 00 00 ...
```

- length = `0e 00` = 14 bytes of data at offset 16
- bytes 16-29 = `52 4d 4f 4d 57 59 49 31 31 34 31 4c 43 56` =
  ASCII `"RMOMWYI1141LCV"` - **the device serial**

This is the serial-number read.

## Decode mapping into our `Status` type

What we can implement today, given the framing is understood for reads
even though the CRC isn't yet cracked for writes:

| Field                | Source                                      | Format                                |
|----------------------|---------------------------------------------|---------------------------------------|
| `serial`             | GET reply to `(0x0D, 0xC8, 0x18)`           | 14 ASCII bytes at offset 16, NUL-terminated within the buffer |
| `firmware_version`   | GET reply to `(0x0D, 0x08, 0x04)`           | 4 bytes at offset 16, reversed -> dotted `a.b.c.d` |
| (device hash / MAC)  | GET reply to `(0x0D, 0x08, 0x18)`           | 24 bytes at offset 16, last 6 = MAC suffix             |
| (full device-info)   | GET reply to `(0x0D, 0x48, 0x19)`           | 34 bytes - includes firmware at offset 20-23, rest TBD |

A `Status` poll for the "Camera Status" surface that
`cameraStatus().tiny.{ai_mode, hdr, face_ae, ...}` exposes is a
different cmd_set+cmd_id pair that this capture doesn't include - it
would be triggered by pressing `0` in interactive mode but the SDK may
cache state and not re-issue the RPC. A targeted `getCameraStatus`
capture is the next pcap to add.

## What's still open

- **CRC at offset 6-7.** Two bytes that change per frame. The same
  request bytes always produce the same CRC, so it's deterministic.
  Brute-forcing common CRC16 variants (CCITT-FALSE, MODBUS, XMODEM,
  ARC, ...) against the four known (header, CRC) pairs below is the
  next analytical step.

  ```
  aa 01 00 00 0c 00 ?? ?? 0a 0d 08 18 00 ...  ->  CRC = 91 5c
  aa 01 01 00 0c 00 ?? ?? 0a 0d 08 04 00 ... ad b6 1b 98 dc 8d 00 ...  ->  CRC = c1 50
  aa 01 02 00 0c 00 ?? ?? 0a 0d 48 19 00 ... ad b6 1b 98 dc 8d 00 ...  ->  CRC = c0 96
  aa 01 03 00 0c 00 ?? ?? 0a 0d c8 18 00 ... 00 ad b6 1b 98 dc 8d 01 01 ...  ->  CRC = 31 53
  ```

  Without a working CRC, libobsbot can only READ via the RPC channel
  (replies don't need us to compute the CRC, only validate it); we
  can't yet issue our own RPC writes there.

- **Bytes 14-15 of every reply.** Look CRC-like but cover a smaller
  range; possibly per-payload checksum or a status code.

- **Selector 0x06 vs selector 0x02 routing inside libdev.so.** Most
  proprietary controls land on the simpler 0x06 mode register
  (`setWdr.md` and siblings). A few - face_focus, status poll, the
  whole handshake - go through 0x02. The criterion is not yet
  documented.
