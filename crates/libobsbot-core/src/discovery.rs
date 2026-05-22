// SPDX-License-Identifier: GPL-3.0-only
//! Device enumeration and hot-plug.

use crate::devices::meet2;
use crate::status::{Event, EventReceiver};
use crate::transport::usb::UsbTransport;
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
    /// Device serial number, when the OS exposes it without opening the
    /// device. Empty when unknown.
    pub serial: String,
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
    ///
    /// Uses `nusb::list_devices` to enumerate every USB device on the system
    /// and filters for the OBSBOT vendor id.
    #[must_use]
    pub fn list(&self) -> Vec<DeviceInfo> {
        use nusb::MaybeFuture;
        let Ok(devices) = nusb::list_devices().wait() else {
            return Vec::new();
        };
        devices
            .filter(|d: &nusb::DeviceInfo| d.vendor_id() == meet2::VENDOR_ID)
            .filter_map(|d| classify(&d))
            .collect()
    }

    /// Find a connected camera by serial number.
    #[must_use]
    pub fn by_serial(&self, sn: &str) -> Option<DeviceInfo> {
        self.list().into_iter().find(|d| d.serial == sn)
    }

    /// Open a connected camera for control.
    pub fn open(&self, info: &DeviceInfo) -> Result<Device> {
        let transport = match info.product_type {
            ProductType::Meet2 => UsbTransport::new(
                info.vendor_id,
                info.product_id,
                meet2::VIDEO_CONTROL_INTERFACE,
                meet2::XU_ENTITY_ID,
                meet2::XU_GUID,
            ),
        };
        Ok(Device::new(info.clone(), Box::new(transport)))
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

fn classify(d: &nusb::DeviceInfo) -> Option<DeviceInfo> {
    let product_type = match d.product_id() {
        meet2::PRODUCT_ID_MEET2 => ProductType::Meet2,
        _ => return None,
    };
    Some(DeviceInfo {
        vendor_id: d.vendor_id(),
        product_id: d.product_id(),
        product_type,
        serial: d.serial_number().unwrap_or_default().to_owned(),
    })
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
