# Firmware upgrade scope

## What's implemented

- `Device::firmware_from_camera` returns the live dotted-decimal
  string (already shipped, see [`getStatus.md`](getStatus.md) for
  the wire format).
- `Device::firmware` returns a parsed [`FirmwareVersion`] so callers
  can compare versions with `<`, `==`, etc. Useful for
  feature-gating code against a minimum firmware.

## What's NOT implemented and why

The full upgrade flow (transfer a firmware blob, reboot into
upgrade mode, poll progress) is intentionally out of scope for
v1. The SDK exposes it via a separate `DevUpgrade` class:

```
DevUpgrade(name, task_type, fw_path, log)
  .setUgPatchDir(...)
  .setFwDir(...)
  .setLogDir(...)
  .setUpgradeType(...)
  .setUgProgressCallback(...)
  .start()
```

Implementing that responsibly requires three things we don't have:

1. **A capture of an actual upgrade running.** The SDK uses
   `DevicePrivate::sendMsgSync` / `sendMsgAsync` with magic numbers
   like `0xb00000000` (`reqDevUpgradeR`) and `0x9000b00070000`
   (`upgradeSetUgModeR`). Decoding those into wire frames requires
   running `convFrmPacketToV0` against each, which I can guess at
   but can't verify without a pcap of OBSBOT's own upgrade flow.

2. **A firmware binary.** Without one I can't validate the upgrade
   actually completes successfully.

3. **The bulk-transfer endpoint.** Firmware blobs are tens of MB;
   they probably don't ride the 60-byte XU RPC channel. The Meet 2
   only has interrupt endpoint 6 on the VideoControl interface
   (see `descriptors.txt`); the upgrade transport is most likely
   a vendor-specific bulk endpoint that appears in an alternate
   interface setting, which we'd have to capture during an upgrade
   to see.

A safer "soft upgrade" surface that exposes only the read paths
(`firmware`, `firmware_from_camera`, `FirmwareVersion`) is what
we ship today. When a capture of OBSBOT's upgrader exists, the
implementation can be filled in incrementally: query state
first (read-only, no firmware needed to test), then the trigger
RPC, then the blob transfer.

## Related symbols (for the next capture session)

In `libdev.so` (`nm` output):

```
Device::isUpgradeNeeded()                 0x39a70    pure local version compare
Device::reqDevUpgradeR(DevMode, int)      0x6a550    sendMsgAsync 0xb00000000
Device::reqDevUpgradeStateR(state&)       0x6a880    sendMsgAsync
Device::reqDevUpgradeResultR(result&)     0x6a770    sendMsgAsync
Device::upgradeSetUgModeR(uint8_t)        0x6b3e0    sendMsgAsync 0x9000b00070000
Device::cameraSetIndicationForUpgradeR()  0x6b2d0    sendMsgAsync 0xb00010000
DevUpgrade::start()                       0x716f0    full flow entry point
DevUpgrade::setFwDir / setLogDir / etc.   accessor scaffolding
```

The natural capture order:

1. `obsbot-cli` (or any libdev-based tool) querying upgrade state
   without actually triggering an upgrade. This gets us the simplest
   RPC bytes.
2. The same tool initiating an upgrade and the bulk transfer that
   follows.
