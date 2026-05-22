// SPDX-License-Identifier: GPL-3.0-only
//! Hot-plug + periodic status events.

use crate::types::Status;

/// Event delivered to consumers of [`crate::Devices::events`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Event {
    /// A new OBSBOT camera was plugged in.
    DeviceAdded {
        /// Serial of the device that connected.
        serial: String,
    },
    /// An OBSBOT camera was unplugged.
    DeviceRemoved {
        /// Serial of the device that disconnected.
        serial: String,
    },
    /// A periodic status sample from the status poller.
    Status {
        /// Serial of the device the status belongs to.
        serial: String,
        /// Status snapshot.
        snapshot: Status,
    },
}

/// Receiver end of the event channel returned by [`crate::Devices::events`].
pub type EventReceiver = crossbeam_channel::Receiver<Event>;

/// Sender end of the event channel; held internally by `Devices` and
/// cloned into each opened `Device` so its status poller can push samples.
pub(crate) type EventSender = crossbeam_channel::Sender<Event>;
