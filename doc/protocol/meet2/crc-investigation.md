# Selector-0x02 CRC: CRACKED (CRC-16/USB, libdev.so disassembly)

The bytes at frame offset `[6,7]` (outer) and `[14,15]` (inner) are
**CRC-16/USB** (poly `0x8005`, init `0xFFFF`, refin and refout both
true, xorout `0xFFFF`).

## How it was solved

1. `libdev.so` ships with debug info and exports a function called
   `calc_crc16` plus two 256-entry tables (`crc16_low_tab`,
   `crc16_high_tab`).
2. Disassembly of `calc_crc16` shows the classic two-table CRC-16
   iteration:

   ```
   index   = crc_lo XOR byte
   crc_lo' = crc_hi XOR high_tab[index]
   crc_hi' = low_tab[index]
   ```

   wrapped with `~init` on entry and `~result` on exit. That's
   identical to CRC-16/USB. The table values confirm
   `table[1] = 0xC0C1`, the MODBUS / USB byte-for-byte pattern.

3. Disassembly of `RmProtocolMsg::frmHeaderProcessForSendV3` shows the
   caller, and how it picks the byte range to CRC:

   ```
   outer_len = *(uint16_t *)(buf + 4)           // always 12
   buf[6..8] = 0
   buf[6..8] = calc_crc16(0, buf, outer_len)    // CRC over buf[0..12]

   if (buf[1] & 0x60) {
       inner_len = *(uint16_t *)(buf + 12)
       buf[14..16] = 0
       buf[14..16] = calc_crc16(0, buf + 12, inner_len + 4)
                                                // CRC over buf[12..16+inner_len]
   }
   ```

   The outer CRC always runs; the inner CRC only when bit 5 or 6 of
   `buf[1]` is set. For the firmware/serial requests (`buf[1] = 0x01`)
   no inner CRC is computed; for face-focus (`buf[1] = 0x25`) one is.

4. Rust reimplementation in `meet2::crc16_usb` + `meet2::build_rpc_frame`
   reproduces all four originally captured frames byte-for-byte. The
   live Meet 2 accepts the synthesised frames the same way it
   accepted the canned ones.

## What's still device-specific

The CRC is now under our control. The MAC tail
(`ad b6 1b 98 dc 8d` for the captured device) is still hard-coded
because we haven't yet learned it from the camera at open time. The
mechanism is straightforward: pair 0 of the handshake (`cmd_id 0x08,
sub-cmd-id 0x18`) returns 24 bytes whose last 6 ARE the MAC, with no
MAC needed in the request. A follow-up will issue this query at
`Device::open` and cache the result instead of using `CAPTURED_MAC`.

## What was tried before the libdev.so trace (preserved for posterity)

- CRC16 catalog: 30 standard variants over 13 byte-range choices, both
  endiannesses. Zero matches. (The right algorithm was in the
  catalog - CRC-16/USB - but our brute had the wrong byte range:
  `buf[0..14]` instead of `buf[0..12]`, because the inner-CRC field
  at `[14,15]` is *outside* the outer CRC's coverage even though it
  sits just past it.)
- Full 16-bit polynomial brute (~17M combos). Zero matches, for the
  same byte-range reason.
- Fletcher / Adler / byte-sum / XOR-of-pairs. Zero matches.

The lesson: read the binary before brute-forcing. The brute-force
tooling worked; we just didn't know the input length was 12 (the
`u16` at `buf[4..6]`) rather than 14.
