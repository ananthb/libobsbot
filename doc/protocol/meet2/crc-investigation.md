# Selector-0x02 CRC investigation: negative result

Bytes 6-7 of every selector-0x02 RPC frame are a content-dependent
checksum (same frame content always produces the same bytes across
sessions on the same device). Cracking this is the prerequisite for
synthesising RPC write requests; the read path needs no CRC from us.

This file records what's been tried, so the next attempt doesn't repeat
the work.

## What's known

- The field is deterministic. Same frame content + sub-seq across three
  separate captures on the same physical Meet 2 produced byte-identical
  CRC values (`91 5c` for handshake message 1 in `initial_apply.pcapng`,
  `setWdr.pcapng`, and `getStatus.pcapng`).
- It's content-dependent. Different sub-seq, cmd_id, or payload bytes
  produce different CRC values.
- The eight known (frame, CRC) pairs are listed in
  [getStatus.md](getStatus.md).

## What's been tried (all negative)

### CRC16 catalog

Tested 30 standard CRC16 variants from the well-known catalog (ARC,
CCITT-FALSE, CDMA2000, DDS-110, DECT-R/X, DNP, EN-13757, GENIBUS, GSM,
IBM-SDLC, ISO-IEC-14443-3-A, KERMIT, LJ1200, MAXIM-DOW, MCRF4XX,
MODBUS, NRSC-5, OPENSAFETY-A/B, PROFIBUS, RIELLO, SPI-FUJITSU,
T10-DIF, TELEDISK, TMS37157, UMTS, USB, XMODEM, ...).

Tested against 13 different "covered byte range" choices (everything
except CRC, everything except CRC with the CRC field zeroed in the
input, just bytes after the CRC, just the header before, the inner
12 bytes claimed by the length field, etc.).

Both little-endian and big-endian interpretations of the CRC field
were tried.

Result: zero matches; zero near-misses (no variant matched even half
the frames).

### Polynomial brute force

Exhaustive sweep:

- All 65536 16-bit polynomials.
- All 4 reflection settings (none / both / refin only / refout only).
- 7 initial values (`0x0000`, `0xFFFF`, plus several known catalog
  init values).
- 3 final-XOR values (`0x0000`, `0xFFFF`, `0x0001`).
- The 4 most plausible byte-range choices.

Total: ~17M combinations, ran in ~3 minutes with table-based CRC16
in Python. Result: zero matches.

### Non-CRC 16-bit checksums

Tried Fletcher-16 (mod 255 and mod 256), Adler-16, byte-sum mod
65536, XOR-of-u16-pairs. Same byte-range choices, same negative result.

## What hasn't been tried (paths forward)

### Algorithm involves the device-specific token

The libdev.so debug log prints `token = d5 be df 2e 46 95` on every
session for this device. The MAC suffix `ad b6 1b 98 dc 8d` is also
embedded in handshake frames. If the CRC is something HMAC-like over
`(content, token)`, brute force over a plain CRC16 space can't find
it.

A test to falsify this hypothesis: capture the same operation on a
**second** physical Meet 2 (different MAC / different token) and
compare CRC bytes for byte-identical content. If they differ, the
token (or some derivative) is part of the CRC input.

### Reveng

[CRC RevEng](https://reveng.sourceforge.io/) reverse-engineers CRC
parameters from sample inputs using GF(2) linear algebra rather than
brute force. It can find non-catalog polynomials and is much more
thorough than our brute. Not in nixpkgs; build from source. Feed it
the eight (input, expected CRC) pairs and see if it can solve.

### Non-linear / multiplicative

If the algorithm uses multiplication, table-driven byte substitution
(like an S-box), or rotations that aren't expressible as a linear CRC,
RevEng won't help either. At that point the only honest path is
either:

- find the algorithm documented somewhere (none of the public
  reverse-engineering projects - `obsbot_tiny_reversing`,
  `meet4k`, `aaronsb/obsbot-camera-control` - have decoded it; they
  all use "replay captured bytes verbatim"); or
- accept the limitation and adopt the same replay-only approach for
  novel operations, while using direct V4L2 + UVC standard controls
  and the simpler selector-0x06 mode register for everything else.

## Practical impact

The decoded surfaces we can already drive don't need this CRC:

- **Standard UVC** (brightness, contrast, saturation, WB, zoom, focus,
  pan/tilt) - goes through V4L2 ioctls, no XU at all.
- **Selector 0x06 mode register** (HDR, media mode, FOV, face AE) -
  flat payload, no CRC field.

The XU surfaces blocked on the CRC:

- Status / firmware / serial / current-state reads. Reply parsing
  works (`getStatus.md` decoded the bytes); we just can't synthesise
  the SET that triggers a fresh reply.
- Face focus, AI mode, auto-framing sub-mode (likely). These ride the
  RPC channel too; uncertain until per-method captures land.

A "replay mode" SDK option that ships canned RPC frames and lets the
camera respond is implementable today, separately from cracking the
CRC, and would unblock status polling. That's the next pragmatic
move once we decide we've exhausted the decode angle.
