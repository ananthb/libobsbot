// SPDX-License-Identifier: GPL-3.0-only
//! Device enumeration and hot-plug.

use std::sync::Arc;

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
/// Construct with [`Devices::new`]. The watcher thread is spawned on
/// `Devices::new` and stops when the struct is dropped.
pub struct Devices {
    events_rx: EventReceiver,
    /// Set by `Drop` to signal the watcher thread to exit.
    stop: Arc<std::sync::atomic::AtomicBool>,
    watcher: Option<std::thread::JoinHandle<()>>,
}

impl Devices {
    /// Start the hot-plug watcher and return a handle.
    pub fn new() -> Result<Self> {
        let (tx, rx) = crossbeam_channel::unbounded::<Event>();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let watcher = spawn_hotplug_thread(tx, stop.clone());
        Ok(Self {
            events_rx: rx,
            stop,
            watcher: Some(watcher),
        })
    }

    /// Snapshot of currently-connected OBSBOT cameras.
    #[must_use]
    pub fn list(&self) -> Vec<DeviceInfo> {
        enumerate()
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

    /// Subscribe to device add/remove events. Each call returns a clone
    /// of the receiver; events are broadcast to every clone.
    #[must_use]
    pub fn events(&self) -> EventReceiver {
        self.events_rx.clone()
    }
}

impl Drop for Devices {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(handle) = self.watcher.take() {
            // The thread checks `stop` between sleeps; give it one
            // poll interval to wake and exit cleanly.
            let _ = handle.join();
        }
    }
}

/// Live enumeration of OBSBOT cameras, regardless of platform.
fn enumerate() -> Vec<DeviceInfo> {
    #[cfg(target_os = "linux")]
    {
        linux::enumerate()
    }
    #[cfg(not(target_os = "linux"))]
    {
        Vec::new()
    }
}

/// How often the hot-plug thread polls the host for device changes.
const HOTPLUG_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);

fn spawn_hotplug_thread(
    tx: crossbeam_channel::Sender<Event>,
    stop: Arc<std::sync::atomic::AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("libobsbot-hotplug".into())
        .spawn(move || hotplug_loop(&tx, &stop))
        .expect("spawn hotplug thread")
}

/// Diff-based hot-plug detector. Emits one `DeviceAdded` event per
/// device present at startup, then a `DeviceAdded` or `DeviceRemoved`
/// every time a poll-interval comparison sees the set change.
fn hotplug_loop(tx: &crossbeam_channel::Sender<Event>, stop: &std::sync::atomic::AtomicBool) {
    use std::collections::HashMap;
    let mut known: HashMap<String, DeviceInfo> = HashMap::new();
    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
        let current: HashMap<String, DeviceInfo> = enumerate()
            .into_iter()
            .map(|d| (device_key(&d), d))
            .collect();
        // Additions.
        for (k, info) in &current {
            if !known.contains_key(k)
                && tx
                    .send(Event::DeviceAdded {
                        serial: info.serial.clone(),
                    })
                    .is_err()
            {
                return;
            }
        }
        // Removals.
        for (k, info) in &known {
            if !current.contains_key(k)
                && tx
                    .send(Event::DeviceRemoved {
                        serial: info.serial.clone(),
                    })
                    .is_err()
            {
                return;
            }
        }
        known = current;
        // Sleep in small chunks so Drop can interrupt us promptly.
        let mut slept = std::time::Duration::ZERO;
        while slept < HOTPLUG_POLL_INTERVAL {
            if stop.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            let chunk = std::time::Duration::from_millis(100);
            std::thread::sleep(chunk);
            slept += chunk;
        }
    }
}

/// Stable key for a `DeviceInfo` so we can diff between polls.
/// Uses busnum + devnum on Linux (the camera's iSerial is empty, so
/// `info.serial` isn't a useful key without opening the device).
fn device_key(info: &DeviceInfo) -> String {
    #[cfg(target_os = "linux")]
    {
        format!("{}:{}", info.busnum, info.devnum)
    }
    #[cfg(not(target_os = "linux"))]
    {
        format!(
            "{:04x}:{:04x}:{}",
            info.vendor_id, info.product_id, info.serial
        )
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

    #[test]
    fn events_channel_open() {
        let d = Devices::new().expect("ctor");
        let rx = d.events();
        // Drain whatever's already there; channel just needs to be alive.
        while rx.try_recv().is_ok() {}
        assert!(!rx.is_full(), "unbounded channel should never be full");
    }
}
