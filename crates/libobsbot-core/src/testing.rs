// SPDX-License-Identifier: GPL-3.0-only
//! Crate-internal test scaffolding.
//!
//! Lets every module write tests against the same `Transport` mocks
//! without re-implementing them. Keep this module narrow - it exists to
//! support tests, not to be a public testing-helpers crate.
//!
//! Patterns:
//!
//! - [`MockTransport`] - records the last `uvc_set` and zero-fills the
//!   `uvc_get` output buffer. Useful for asserting which entity/selector/
//!   payload a `Device` method calls without caring what comes back.
//!
//! - [`ScriptedTransport`] - returns pre-staged byte vectors from
//!   successive `uvc_get` calls. Useful for testing methods that issue
//!   multiple GETs (e.g. range queries that ask for `Min` then `Max`).
//!
//! - [`device_with_mock`] / [`device_with_scripted_get`] - convenience
//!   constructors that wrap a `Transport` mock in a `Device` and (where
//!   relevant) hand back an `Arc<MockTransport>` for assertions.

#![cfg(test)]

use std::sync::{Arc, Mutex};

use crate::device::Device;
use crate::devices::meet2;
use crate::discovery::DeviceInfo;
use crate::transport::Transport;
use crate::types::ProductType;
use crate::uvc::UvcGet;
use crate::Result;

/// Records the last `uvc_set` triple and zero-fills `uvc_get` output.
#[derive(Default)]
pub(crate) struct MockTransport {
    pub last_set: Mutex<Option<(u8, u8, Vec<u8>)>>,
}

impl Transport for MockTransport {
    fn uvc_set(&self, entity: u8, selector: u8, payload: &[u8]) -> Result<()> {
        *self.last_set.lock().unwrap() = Some((entity, selector, payload.to_vec()));
        Ok(())
    }

    fn uvc_get(&self, _req: UvcGet, _entity: u8, _selector: u8, out: &mut [u8]) -> Result<usize> {
        for b in &mut *out {
            *b = 0;
        }
        Ok(out.len())
    }
}

/// `Transport` whose `uvc_set` is recorded on the wrapped [`MockTransport`].
///
/// Lets a test hold an `Arc<MockTransport>` for assertions while the
/// `Device` consumes the boxed `Transport` impl. Construct via
/// [`device_with_mock`].
pub(crate) struct Forward(pub Arc<MockTransport>);

impl Transport for Forward {
    fn uvc_set(&self, entity: u8, selector: u8, payload: &[u8]) -> Result<()> {
        self.0.uvc_set(entity, selector, payload)
    }
    fn uvc_get(&self, req: UvcGet, entity: u8, selector: u8, out: &mut [u8]) -> Result<usize> {
        self.0.uvc_get(req, entity, selector, out)
    }
}

/// Build a [`Device`] that routes through a shared [`MockTransport`].
/// Returns both the device handle and the `Arc<MockTransport>` for
/// last-call assertions.
pub(crate) fn device_with_mock() -> (Device, Arc<MockTransport>) {
    let mock = Arc::new(MockTransport::default());
    let transport: Arc<dyn Transport> = Arc::new(Forward(mock.clone()));
    (Device::new(meet2_mock_info(), transport, None), mock)
}

/// [`DeviceInfo`] suitable for tests against a fake Meet 2.
pub(crate) fn meet2_mock_info() -> DeviceInfo {
    DeviceInfo {
        vendor_id: meet2::VENDOR_ID,
        product_id: meet2::PRODUCT_ID_MEET2,
        product_type: ProductType::Meet2,
        serial: "MOCK".to_owned(),
        #[cfg(target_os = "linux")]
        busnum: 0,
        #[cfg(target_os = "linux")]
        devnum: 0,
    }
}

/// `Transport` whose `uvc_get` returns scripted responses in order.
///
/// Useful when a test method does several `uvc_get` calls (e.g. range
/// queries that issue `Min` then `Max`) and the expected return values
/// differ.
pub(crate) struct ScriptedTransport {
    responses: Mutex<Vec<Vec<u8>>>,
}

impl ScriptedTransport {
    /// `responses` are consumed in the order written - first response goes
    /// to the first `uvc_get` call.
    pub(crate) fn new(mut responses: Vec<Vec<u8>>) -> Self {
        responses.reverse(); // Vec::pop pulls from the end.
        Self {
            responses: Mutex::new(responses),
        }
    }
}

impl Transport for ScriptedTransport {
    fn uvc_set(&self, _entity: u8, _selector: u8, _payload: &[u8]) -> Result<()> {
        Ok(())
    }

    fn uvc_get(&self, _req: UvcGet, _entity: u8, _selector: u8, out: &mut [u8]) -> Result<usize> {
        let mut responses = self.responses.lock().unwrap();
        let Some(next) = responses.pop() else {
            for b in &mut *out {
                *b = 0;
            }
            return Ok(out.len());
        };
        let n = next.len().min(out.len());
        out[..n].copy_from_slice(&next[..n]);
        Ok(n)
    }
}

/// Build a [`Device`] backed by a [`ScriptedTransport`]. Responses are
/// consumed in insertion order - the first vector goes to the first
/// `uvc_get` call.
pub(crate) fn device_with_scripted_get(responses: Vec<Vec<u8>>) -> Device {
    let transport: Arc<dyn Transport> = Arc::new(ScriptedTransport::new(responses));
    Device::new(meet2_mock_info(), transport, None)
}

/// Convenience for `last_set` assertions - panics with a clear message if
/// no SET has happened yet.
pub(crate) fn last_set(mock: &Arc<MockTransport>) -> (u8, u8, Vec<u8>) {
    mock.last_set
        .lock()
        .unwrap()
        .clone()
        .expect("expected a uvc_set call, but none was recorded")
}
