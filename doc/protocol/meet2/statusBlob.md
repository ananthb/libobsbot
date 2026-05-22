# XU mode-register status blob (selector 0x06 GET)

`GET_CUR` on entity 2, selector `0x06` returns a 60-byte status
snapshot reflecting the camera's current state for several XU
controls. The first byte is a fixed marker (`0x27` on firmware
4.4.6.1); subsequent offsets hold one or more control values.

Mapped via toggle-and-diff experiments on a real Meet 2 (the disposable
`xu_status_decode` example, deleted after the decode landed):

| Offset | Field        | Encoding                                |
|--------|--------------|-----------------------------------------|
| 0x00   | marker       | always `0x27` on observed firmware      |
| 0x06   | WDR          | `0` off, `1` on                         |
| 0x07   | Face-AE      | `0` off, `1` on                         |
| 0x18   | AI / framing | shared byte: `0` baseline; AI mode      |
|        | / media mode | enums (1-5); also bumps when            |
|        |              | `set_auto_framing` or                   |
|        |              | `set_media_mode(AutoFrame)` runs        |
| 0x20   | (mirror)     | shares state with offset 0x18, with a   |
|        |              | high-nibble encoding of the AI mode and |
|        |              | a low-nibble close-upper bit            |

The Meet 2 collapses **AI master mode**, **auto-framing sub-mode**,
and `MediaMode::AutoFrame` into a single internal "AI work mode"
enum, so the status byte at `0x18` represents whichever one was most
recently set. The dedicated public methods (`Device::ai_mode`,
`auto_framing`, `media_mode`) all read this byte and interpret it
through their own enum.

Things that DID NOT show up in the diff:

- **FOV preset** - `set_fov(Wide / Medium / Narrow)` writes to the
  mode-register selector and the camera acknowledges, but no byte in
  this status blob changed. FOV state likely lives in a different
  query path (probably a selector 0x02 RPC) or is purely a one-shot
  config that doesn't persist into this blob.

- **face_focus** - rides the selector 0x02 RPC channel for SETs, not
  the mode-register selector. Its state probably also lives in an RPC
  reply we haven't captured yet.

Bytes at offsets 0x09, 0x0a, 0x16, 0x21, 0x26, 0x27 hold non-zero
values (`0x03`, `0x78`, `0x21`, `0x03`, `0x8d`, `0x0c` on the
captured device) but didn't change under any of the toggle tests.
They're either device identifiers, firmware-version bytes, or
parameters that didn't get exercised; decoding them isn't gating any
public API.
