// SPDX-License-Identifier: GPL-3.0-only
//! Device enumeration and hot-plug.

use crate::status::{Event, EventReceiver};
use crate::types::ProductType;
use crate::{Device, Result};

/// Description of a connected OBSBOT camera that has not been opened yet.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// USB vendor id.
    pub vendor_id: u16,
    /// USB product id.
    pub product_id: u16,
    /// Camera model.
    pub product_type: ProductType,
    /// Device serial number, when the OS exposes it. Empty when unknown
    /// (the Meet 2 sets `iSerial = 0` in its USB descriptor, so no serial
    /// is available before opening; the camera reports one at runtime).
    pub serial: String,

    #[cfg(target_os = "linux")]
    pub(crate) busnum: u8,
    #[cfg(target_os = "linux")]
    pub(crate) devnum: u8,
}

/// Owns the hot-plug watcher and the registry of connected cameras.
///
/// Construct with [`Devices::new`]. Drop the handle to stop the watcher
/// thread (no thread is spawned in v0.0.0 — added in M7).
pub struct Devices {
    // No state in v0.0.0. The hot-plug watcher thread + status registry land
    // in M7. The struct is kept so that the public API is stable across the
    // milestone progression.
    _private: (),
}

impl Devices {
    /// Start the hot-plug watcher and return a handle.
    pub fn new() -> Result<Self> {
        Ok(Self { _private: () })
    }

    /// Snapshot of currently-connected OBSBOT cameras.
    #[must_use]
    pub fn list(&self) -> Vec<DeviceInfo> {
        #[cfg(target_os = "linux")]
        {
            linux::enumerate()
        }
        #[cfg(not(target_os = "linux"))]
        {
            Vec::new()
        }
    }

    /// Find a connected camera by serial number.
    #[must_use]
    pub fn by_serial(&self, sn: &str) -> Option<DeviceInfo> {
        self.list().into_iter().find(|d| d.serial == sn)
    }

    /// Open a connected camera for control.
    pub fn open(&self, info: &DeviceInfo) -> Result<Device> {
        #[cfg(target_os = "linux")]
        {
            let transport = crate::transport::usb::UsbTransport::open(info)?;
            Ok(Device::new(info.clone(), Box::new(transport)))
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = info;
            Err(crate::Error::Unsupported(
                "open: macOS and Windows transports are planned; only Linux is implemented",
            ))
        }
    }

    /// Subscribe to device add/remove and periodic status events.
    ///
    /// The receiver is currently never sent to — the hot-plug watcher lands
    /// in M7. Returned eagerly so consumers can wire their event loops now.
    #[must_use]
    pub fn events(&self) -> EventReceiver {
        let (_tx, rx) = crossbeam_channel::unbounded::<Event>();
        rx
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::DeviceInfo;
    use crate::devices::meet2;
    use crate::types::ProductType;
    use std::collections::HashMap;

    /// Walk `/sys/class/video4linux/` and return one entry per OBSBOT USB
    /// device. Multiple v4l2 nodes for the same camera are collapsed by
    /// `(busnum, devnum)`.
    pub(super) fn enumerate() -> Vec<DeviceInfo> {
        let Ok(dir) = std::fs::read_dir("/sys/class/video4linux") else {
            return Vec::new();
        };
        let mut by_dev: HashMap<(u8, u8), DeviceInfo> = HashMap::new();
        for entry in dir.flatten() {
            let device_link = entry.path().join("device");
            let Ok(iface_dir) = std::fs::canonicalize(&device_link) else {
                continue;
            };
            let Some(dev_dir) = iface_dir.parent() else {
                continue;
            };
            let (Ok(busnum), Ok(devnum), Ok(vendor_id), Ok(product_id)) = (
                read_u8(&dev_dir.join("busnum")),
                read_u8(&dev_dir.join("devnum")),
                read_u16_hex(&dev_dir.join("idVendor")),
                read_u16_hex(&dev_dir.join("idProduct")),
            ) else {
                continue;
            };
            if vendor_id != meet2::VENDOR_ID {
                continue;
            }
            let product_type = match product_id {
                meet2::PRODUCT_ID_MEET2 => ProductType::Meet2,
                _ => continue,
            };
            let serial = std::fs::read_to_string(dev_dir.join("serial"))
                .unwrap_or_default()
                .trim()
                .to_owned();
            by_dev.entry((busnum, devnum)).or_insert(DeviceInfo {
                vendor_id,
                product_id,
                product_type,
                serial,
                busnum,
                devnum,
            });
        }
        by_dev.into_values().collect()
    }

    fn read_u8(path: &std::path::Path) -> crate::Result<u8> {
        let s = std::fs::read_to_string(path)
            .map_err(|e| crate::Error::Usb(format!("read {}: {e}", path.display())))?;
        s.trim()
            .parse()
            .map_err(|_| crate::Error::Usb(format!("parse u8 from {}", path.display())))
    }

    fn read_u16_hex(path: &std::path::Path) -> crate::Result<u16> {
        let s = std::fs::read_to_string(path)
            .map_err(|e| crate::Error::Usb(format!("read {}: {e}", path.display())))?;
        u16::from_str_radix(s.trim(), 16)
            .map_err(|_| crate::Error::Usb(format!("parse u16 hex from {}", path.display())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_does_not_panic_without_hardware() {
        let d = Devices::new().expect("ctor");
        let _ = d.list();
    }

    #[test]
    fn by_serial_returns_none_when_empty() {
        let d = Devices::new().expect("ctor");
        assert!(d.by_serial("nonexistent").is_none());
    }
}
