// SPDX-License-Identifier: GPL-3.0-only
//! macOS USB transport via `IOKit`.
//!
//! Talks to the OBSBOT camera through `IOUSBDeviceInterface::DeviceRequest`
//! on the device's default control pipe (endpoint 0). Class-specific
//! UVC transfers - `SET_CUR` / `GET_*` on a `(entity, selector)` pair -
//! are routed by `wIndex = (entity << 8) | interface_number` and do
//! NOT require `USBDeviceOpen` (per Apple's `DeviceRequestTO` docs:
//! "the device does not have to be open to use this function").
//!
//! Crucially, this bypasses the interface-level claim that
//! `UVCAssistant` (the Apple system extension that auto-attaches to
//! every UVC device on Big Sur and above) holds on the
//! `VideoControl` interface. `IOUSBInterfaceInterface::ControlRequest`
//! would have to fight `UVCAssistant` for that claim - and lose, even
//! through `USBInterfaceOpenSeize`, because `UVCAssistant` refuses to
//! yield. Device-level `DeviceRequest` sidesteps the conflict.
//!
//! Streaming would still need the interface, but this SDK only does
//! control transfers; we never claim a pipe.

#![cfg(target_os = "macos")]

use std::ffi::c_void;
use std::sync::Mutex;

use core_foundation::base::{CFType, TCFType};
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_foundation_sys::base::kCFAllocatorDefault;
use core_foundation_sys::uuid::{
    CFUUIDBytes, CFUUIDGetConstantUUIDWithBytes, CFUUIDGetUUIDBytes, CFUUIDRef,
};
use io_kit_sys::types::{io_iterator_t, io_object_t, io_registry_entry_t, io_service_t};
use io_kit_sys::usb::lib::kIOUSBDeviceClassName;
use io_kit_sys::{
    kIOMasterPortDefault, IOIteratorNext, IOObjectRelease, IORegistryEntryCreateCFProperty,
    IORegistryEntryGetRegistryEntryID, IORegistryEntryIDMatching, IOServiceGetMatchingService,
    IOServiceGetMatchingServices, IOServiceMatching,
};

use crate::devices::meet2;
use crate::types::ProductType;
use crate::uvc::UvcGet;
use crate::{Error, Result};

// ---- IOKit USB inline FFI -------------------------------------------------
//
// Apple deprecated the user-space `IOUSBLib.h` struct definitions in
// modern SDKs and now ships only the UUID `#define`s. The COM-style
// vtable ABI is still stable for runtime use; we declare just the
// methods we need, with every preceding vtable slot present so the
// field offsets match Apple's binary contract.

/// `kIOReturnSuccess` from `IOReturn.h`.
const KERN_SUCCESS: i32 = 0;
const KIO_RETURN_EXCLUSIVE_ACCESS: i32 = -0x1fff_fd3b; // 0xe000_02c5

/// Sentinel for "don't filter on this field" in `IOUSBFindInterfaceRequest`.
const KUSB_FIND_INTERFACE_DONT_CARE: u16 = 0xffff;

/// UVC `VideoControl` interface descriptor values.
const UVC_INTERFACE_CLASS_VIDEO: u16 = 0x0e;
const UVC_INTERFACE_SUBCLASS_VIDEO_CONTROL: u16 = 0x01;

#[repr(C)]
struct IOUSBDevRequest {
    bm_request_type: u8,
    b_request: u8,
    w_value: u16,
    w_index: u16,
    w_length: u16,
    p_data: *mut c_void,
    w_len_done: u32,
}

#[repr(C)]
#[allow(clippy::struct_field_names)]
struct IOUSBFindInterfaceRequest {
    b_interface_class: u16,
    b_interface_sub_class: u16,
    b_interface_protocol: u16,
    b_alternate_setting: u16,
}

/// `IOUSBDeviceInterface` v100 vtable. Declares enough slots to reach
/// `CreateInterfaceIterator`; later versions append after this and
/// `QueryInterface` would give us a fatter struct, but the prefix is
/// ABI-stable.
#[repr(C)]
#[allow(dead_code)] // The unused vtable slots are real ABI we just don't call.
struct IOUSBDeviceInterface {
    _reserved: *mut c_void,
    query_interface: unsafe extern "C" fn(*mut c_void, CFUUIDBytes, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "C" fn(*mut c_void) -> u32,
    release: unsafe extern "C" fn(*mut c_void) -> u32,
    _create_device_async_event_source: unsafe extern "C" fn(*mut c_void, *mut *const c_void) -> i32,
    _get_device_async_event_source: unsafe extern "C" fn(*mut c_void) -> *const c_void,
    _create_device_async_port: unsafe extern "C" fn(*mut c_void, *mut u32) -> i32,
    _get_device_async_port: unsafe extern "C" fn(*mut c_void) -> u32,
    _usb_device_open: unsafe extern "C" fn(*mut c_void) -> i32,
    _usb_device_close: unsafe extern "C" fn(*mut c_void) -> i32,
    _get_device_class: unsafe extern "C" fn(*mut c_void, *mut u8) -> i32,
    _get_device_sub_class: unsafe extern "C" fn(*mut c_void, *mut u8) -> i32,
    _get_device_protocol: unsafe extern "C" fn(*mut c_void, *mut u8) -> i32,
    _get_device_vendor: unsafe extern "C" fn(*mut c_void, *mut u16) -> i32,
    _get_device_product: unsafe extern "C" fn(*mut c_void, *mut u16) -> i32,
    _get_device_release_number: unsafe extern "C" fn(*mut c_void, *mut u16) -> i32,
    _get_device_address: unsafe extern "C" fn(*mut c_void, *mut u16) -> i32,
    _get_device_bus_power_available: unsafe extern "C" fn(*mut c_void, *mut u32) -> i32,
    _get_device_speed: unsafe extern "C" fn(*mut c_void, *mut u8) -> i32,
    _get_number_of_configurations: unsafe extern "C" fn(*mut c_void, *mut u8) -> i32,
    _get_location_id: unsafe extern "C" fn(*mut c_void, *mut u32) -> i32,
    _get_configuration_descriptor_ptr:
        unsafe extern "C" fn(*mut c_void, u8, *mut *const c_void) -> i32,
    _get_configuration: unsafe extern "C" fn(*mut c_void, *mut u8) -> i32,
    _set_configuration: unsafe extern "C" fn(*mut c_void, u8) -> i32,
    _get_bus_frame_number: unsafe extern "C" fn(*mut c_void, *mut u64, *mut u64) -> i32,
    _reset_device: unsafe extern "C" fn(*mut c_void) -> i32,
    device_request: unsafe extern "C" fn(*mut c_void, *mut IOUSBDevRequest) -> i32,
    _device_request_async:
        unsafe extern "C" fn(*mut c_void, *mut IOUSBDevRequest, *mut c_void, *mut c_void) -> i32,
    create_interface_iterator: unsafe extern "C" fn(
        *mut c_void,
        *mut IOUSBFindInterfaceRequest,
        *mut io_iterator_t,
    ) -> i32,
}

/// `IOCFPlugInInterface` vtable (we only need it to call
/// `QueryInterface` on the plugin pointer returned by
/// `IOCreatePlugInInterfaceForService`).
#[repr(C)]
#[allow(dead_code)]
struct IOCFPlugInInterface {
    _reserved: *mut c_void,
    query_interface: unsafe extern "C" fn(*mut c_void, CFUUIDBytes, *mut *mut c_void) -> i32,
    _add_ref: unsafe extern "C" fn(*mut c_void) -> u32,
    release: unsafe extern "C" fn(*mut c_void) -> u32,
    _version: u16,
    _revision: u16,
    _probe: unsafe extern "C" fn(*mut c_void, *const c_void, io_service_t, *mut i32) -> i32,
    _start: unsafe extern "C" fn(*mut c_void, *const c_void, io_service_t) -> i32,
    _stop: unsafe extern "C" fn(*mut c_void) -> i32,
}

extern "C" {
    fn IOCreatePlugInInterfaceForService(
        service: io_service_t,
        plugin_type: CFUUIDRef,
        interface_type: CFUUIDRef,
        plugin_interface: *mut *mut *mut IOCFPlugInInterface,
        score: *mut i32,
    ) -> i32;
}

/// `kIOUSBDeviceUserClientTypeID` (9dc7b780-9ec0-11d4-a54f-000a27052861).
fn k_iousb_device_user_client_type_id() -> CFUUIDRef {
    // SAFETY: `CFUUIDGetConstantUUIDWithBytes` returns a process-wide
    // singleton; safe to call from any thread.
    unsafe {
        CFUUIDGetConstantUUIDWithBytes(
            std::ptr::null(),
            0x9d,
            0xc7,
            0xb7,
            0x80,
            0x9e,
            0xc0,
            0x11,
            0xd4,
            0xa5,
            0x4f,
            0x00,
            0x0a,
            0x27,
            0x05,
            0x28,
            0x61,
        )
    }
}

/// `kIOCFPlugInInterfaceID` (c244e858-109c-11d4-91d4-0050e4c6426f).
fn k_iocf_plugin_interface_id() -> CFUUIDRef {
    // SAFETY: see `k_iousb_device_user_client_type_id`.
    unsafe {
        CFUUIDGetConstantUUIDWithBytes(
            std::ptr::null(),
            0xc2,
            0x44,
            0xe8,
            0x58,
            0x10,
            0x9c,
            0x11,
            0xd4,
            0x91,
            0xd4,
            0x00,
            0x50,
            0xe4,
            0xc6,
            0x42,
            0x6f,
        )
    }
}

/// `kIOUSBDeviceInterfaceID` (5c8187d0-9ef3-11d4-8b45-000a27052861).
/// The v100 interface; every later version is a superset, so a fatter
/// vtable returned at runtime is fine - we never touch the slots past
/// v100.
fn k_iousb_device_interface_id() -> CFUUIDRef {
    // SAFETY: see `k_iousb_device_user_client_type_id`.
    unsafe {
        CFUUIDGetConstantUUIDWithBytes(
            std::ptr::null(),
            0x5c,
            0x81,
            0x87,
            0xd0,
            0x9e,
            0xf3,
            0x11,
            0xd4,
            0x8b,
            0x45,
            0x00,
            0x0a,
            0x27,
            0x05,
            0x28,
            0x61,
        )
    }
}

// ---- Transport ------------------------------------------------------------

/// `IOKit`-backed transport. Owns the `IOUSBDeviceInterface` for the
/// camera and the resolved `VideoControl` interface number.
///
/// The interface pointer is a COM-style `(IOUSBDeviceInterface **)`.
/// Apple's docs state that calls on it are thread-safe; the Rust type
/// system can't see that, so we wrap it in a `Mutex` for an
/// unambiguous Send + Sync contract. Our access pattern (one short
/// call per `uvc_set` / `uvc_get`) doesn't need contention-free fast
/// paths.
pub struct MacosTransport {
    inner: Mutex<TransportInner>,
}

struct TransportInner {
    /// `IOUSBDeviceInterface **` returned by `QueryInterface`.
    dev: *mut *mut IOUSBDeviceInterface,
    /// `bInterfaceNumber` of the `VideoControl` interface. Encoded
    /// into `wIndex` on every control transfer (UVC class-specific
    /// requests are addressed by interface number, not by entity id).
    interface_number: u8,
}

impl Drop for TransportInner {
    fn drop(&mut self) {
        if self.dev.is_null() {
            return;
        }
        // SAFETY: `self.dev` is a live COM pointer; releasing it
        // decrements the COM refcount to zero.
        unsafe {
            let vt = *self.dev;
            ((*vt).release)(self.dev.cast());
        }
    }
}

impl MacosTransport {
    /// Open the OBSBOT camera identified by `info`. Re-finds the device
    /// in `IOKit` via its registry id, builds an `IOUSBDeviceInterface`,
    /// and resolves the `VideoControl` interface number for later
    /// control transfers. Does NOT call `USBDeviceOpen`: class-specific
    /// `DeviceRequest`s don't require an exclusive device claim and we
    /// don't want to fight `UVCAssistant`.
    pub fn open(info: &crate::discovery::DeviceInfo) -> Result<Self> {
        let device_service = lookup_service_by_registry_id(info.registry_id)?;
        let _device_release = ServiceRelease(device_service);

        let dev = create_device_interface(device_service)?;

        let interface_number = match find_video_control_interface_number(dev) {
            Ok(n) => n,
            Err(e) => {
                // Release the device interface before bailing.
                // SAFETY: `dev` came from `QueryInterface`.
                unsafe {
                    ((**dev).release)(dev.cast());
                }
                return Err(e);
            }
        };

        tracing::debug!(
            vid = format_args!("{:04x}", info.vendor_id),
            pid = format_args!("{:04x}", info.product_id),
            registry_id = format_args!("{:#x}", info.registry_id),
            interface_number,
            "opened IOKit USB transport",
        );

        Ok(Self {
            inner: Mutex::new(TransportInner {
                dev,
                interface_number,
            }),
        })
    }
}

impl super::Transport for MacosTransport {
    fn uvc_set(&self, entity: u8, selector: u8, payload: &[u8]) -> Result<()> {
        tracing::trace!(
            entity,
            selector = format_args!("{selector:#04x}"),
            len = payload.len(),
            "macos uvc_set",
        );
        let inner = self.inner.lock().expect("transport mutex poisoned");
        let mut buf = payload.to_vec();
        let mut req = build_request(
            UvcRequestDirection::HostToDevice,
            0x01, // SET_CUR
            entity,
            selector,
            inner.interface_number,
            &mut buf,
        )?;
        let kr = device_request(inner.dev, &mut req)?;
        if kr != KERN_SUCCESS {
            return Err(io_return_error("DeviceRequest SET_CUR", kr));
        }
        Ok(())
    }

    fn uvc_get(&self, req: UvcGet, entity: u8, selector: u8, out: &mut [u8]) -> Result<usize> {
        tracing::trace!(
            entity,
            selector = format_args!("{selector:#04x}"),
            req = ?req,
            length = out.len(),
            "macos uvc_get",
        );
        let inner = self.inner.lock().expect("transport mutex poisoned");
        let mut dev_req = build_request(
            UvcRequestDirection::DeviceToHost,
            req as u8,
            entity,
            selector,
            inner.interface_number,
            out,
        )?;
        let kr = device_request(inner.dev, &mut dev_req)?;
        if kr != KERN_SUCCESS {
            return Err(io_return_error("DeviceRequest GET_*", kr));
        }
        Ok(usize::try_from(dev_req.w_len_done).unwrap_or(out.len()))
    }
}

#[derive(Clone, Copy)]
enum UvcRequestDirection {
    HostToDevice,
    DeviceToHost,
}

/// Build a UVC class-specific `IOUSBDevRequest` targeting
/// `(entity, selector)` on the given `interface_number`. Returns an
/// error if `data` doesn't fit in `wLength`.
fn build_request(
    direction: UvcRequestDirection,
    b_request: u8,
    entity: u8,
    selector: u8,
    interface_number: u8,
    data: &mut [u8],
) -> Result<IOUSBDevRequest> {
    let length = u16::try_from(data.len()).map_err(|_| {
        Error::Usb(format!(
            "macos: control transfer length {} exceeds u16::MAX",
            data.len()
        ))
    })?;
    let bm_request_type = match direction {
        // class-specific, recipient = interface
        UvcRequestDirection::HostToDevice => 0x21,
        UvcRequestDirection::DeviceToHost => 0xa1,
    };
    Ok(IOUSBDevRequest {
        bm_request_type,
        b_request,
        w_value: u16::from(selector) << 8,
        w_index: (u16::from(entity) << 8) | u16::from(interface_number),
        w_length: length,
        p_data: data.as_mut_ptr().cast(),
        w_len_done: 0,
    })
}

/// Issue a single control request via the device interface vtable.
fn device_request(dev: *mut *mut IOUSBDeviceInterface, req: &mut IOUSBDevRequest) -> Result<i32> {
    if dev.is_null() {
        return Err(Error::Usb("macos: device interface pointer is null".into()));
    }
    // SAFETY: `dev` is a live `IOUSBDeviceInterface **` we obtained in
    // `MacosTransport::open` and have not released yet; the request
    // struct is fully initialised.
    let kr = unsafe { ((**dev).device_request)(dev.cast(), req) };
    Ok(kr)
}

fn io_return_error(stage: &str, kr: i32) -> Error {
    let detail = if kr == KIO_RETURN_EXCLUSIVE_ACCESS {
        "kIOReturnExclusiveAccess - another app (FaceTime / Zoom / OBS / Cheese) is using the camera in a way that blocks raw USB control"
    } else {
        ""
    };
    Error::Usb(format!(
        "macos: {stage} returned IOReturn {kr:#010x} {detail}"
    ))
}

// ---- open() helpers -------------------------------------------------------

/// Re-find the device by registry id and return its `io_service_t`.
fn lookup_service_by_registry_id(registry_id: u64) -> Result<io_service_t> {
    // SAFETY: `IORegistryEntryIDMatching` returns a +1 retained dict
    // that `IOServiceGetMatchingService` consumes for us.
    let dict = unsafe { IORegistryEntryIDMatching(registry_id) };
    if dict.is_null() {
        return Err(Error::NotFound);
    }
    let svc = unsafe { IOServiceGetMatchingService(kIOMasterPortDefault, dict) };
    if svc == 0 {
        return Err(Error::NotFound);
    }
    Ok(svc)
}

/// Walk the device's interfaces looking for `(class=Video,
/// subclass=VideoControl)` and read its `bInterfaceNumber` directly
/// from the registry entry - no `USBInterfaceOpen` required.
fn find_video_control_interface_number(dev: *mut *mut IOUSBDeviceInterface) -> Result<u8> {
    let mut filter = IOUSBFindInterfaceRequest {
        b_interface_class: UVC_INTERFACE_CLASS_VIDEO,
        b_interface_sub_class: UVC_INTERFACE_SUBCLASS_VIDEO_CONTROL,
        b_interface_protocol: KUSB_FIND_INTERFACE_DONT_CARE,
        b_alternate_setting: KUSB_FIND_INTERFACE_DONT_CARE,
    };
    let mut iter: io_iterator_t = 0;
    // SAFETY: `dev` is a live device interface; we wrote `filter` and
    // pass a valid output pointer.
    let kr =
        unsafe { ((**dev).create_interface_iterator)(dev.cast(), &raw mut filter, &raw mut iter) };
    if kr != KERN_SUCCESS || iter == 0 {
        return Err(Error::Usb(format!(
            "macos: CreateInterfaceIterator returned {kr:#010x}"
        )));
    }
    let iter_obj = IOObject(iter);

    // The first hit is the VideoControl interface (interface 0 on the
    // Meet 2 per the committed descriptors). We still read
    // bInterfaceNumber dynamically in case a future model places it
    // somewhere else.
    //
    // SAFETY: `iter_obj.0` is a live iterator until we drop it.
    let svc = unsafe { IOIteratorNext(iter_obj.0) };
    if svc == 0 {
        return Err(Error::Usb(
            "macos: no VideoControl interface on this device".into(),
        ));
    }
    let svc = IOObject(svc);
    let n = read_u32_property(svc.as_raw(), "bInterfaceNumber")
        .and_then(|v| u8::try_from(v).ok())
        .ok_or_else(|| {
            Error::Usb("macos: bInterfaceNumber missing on VideoControl interface".into())
        })?;
    Ok(n)
}

/// `IOCreatePlugInInterfaceForService` + `QueryInterface` to produce an
/// `IOUSBDeviceInterface **`.
fn create_device_interface(service: io_service_t) -> Result<*mut *mut IOUSBDeviceInterface> {
    let mut plugin: *mut *mut IOCFPlugInInterface = std::ptr::null_mut();
    let mut score: i32 = 0;
    // SAFETY: `service` is a live io_service_t; the UUIDs are static
    // process-wide constants.
    let kr = unsafe {
        IOCreatePlugInInterfaceForService(
            service,
            k_iousb_device_user_client_type_id(),
            k_iocf_plugin_interface_id(),
            &raw mut plugin,
            &raw mut score,
        )
    };
    if kr != KERN_SUCCESS || plugin.is_null() {
        return Err(Error::Usb(format!(
            "macos: IOCreatePlugInInterfaceForService returned {kr:#010x}"
        )));
    }
    let mut iface: *mut c_void = std::ptr::null_mut();
    // SAFETY: `plugin` is a live IOCFPlugInInterface; `QueryInterface`
    // is a stable COM contract.
    let qkr = unsafe {
        let bytes = CFUUIDGetUUIDBytes(k_iousb_device_interface_id());
        ((**plugin).query_interface)(plugin.cast(), bytes, &raw mut iface)
    };
    // Whether QueryInterface succeeded or not we no longer need the
    // plugin interface itself.
    // SAFETY: `plugin` is a live COM pointer.
    unsafe {
        ((**plugin).release)(plugin.cast());
    }
    if qkr != KERN_SUCCESS || iface.is_null() {
        return Err(Error::Usb(format!(
            "macos: QueryInterface(IOUSBDeviceInterface) returned HRESULT {qkr:#010x}"
        )));
    }
    Ok(iface.cast::<*mut IOUSBDeviceInterface>())
}

/// SAFETY: The `IOUSBDeviceInterface` is Apple-documented as safe to
/// call from any thread. We wrap it in a `Mutex` in `MacosTransport`
/// for an unambiguous contract.
unsafe impl Send for TransportInner {}
unsafe impl Sync for TransportInner {}

// ---- RAII wrappers --------------------------------------------------------

/// RAII wrapper around an `IOKit` `io_object_t`. Calls `IOObjectRelease`
/// on drop. The wrapped value is `0` when the object has been consumed
/// (so dropping is a no-op).
struct IOObject(io_object_t);

impl Drop for IOObject {
    fn drop(&mut self) {
        if self.0 != 0 {
            // SAFETY: `self.0` came from an `IOKit` "create" or "copy"
            // function and we have not handed ownership elsewhere.
            unsafe {
                IOObjectRelease(self.0);
            }
        }
    }
}

impl IOObject {
    fn as_raw(&self) -> io_object_t {
        self.0
    }
}

/// Releases an `io_service_t` on drop. Same contract as [`IOObject`]
/// but typed as a `service` for readability at call sites.
struct ServiceRelease(io_service_t);

impl Drop for ServiceRelease {
    fn drop(&mut self) {
        if self.0 != 0 {
            // SAFETY: see `IOObject::drop`.
            unsafe {
                IOObjectRelease(self.0);
            }
        }
    }
}

// ---- enumeration ----------------------------------------------------------

/// Enumerate OBSBOT cameras via `IOKit`. Walks `IOUSBDevice` services
/// and emits one [`crate::discovery::DeviceInfo`] per device whose
/// vendor + product id matches a model we recognise.
pub(crate) fn enumerate() -> Vec<crate::discovery::DeviceInfo> {
    let Some(iter) = matching_iter() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    loop {
        // SAFETY: `iter.as_raw()` is a live `IOKit` iterator.
        let raw = unsafe { IOIteratorNext(iter.as_raw()) };
        if raw == 0 {
            break;
        }
        let service = IOObject(raw);
        if let Some(info) = device_info_from_service(service.as_raw()) {
            out.push(info);
        }
    }
    out
}

/// Build the `IOUSBDevice` matching dictionary and run
/// `IOServiceGetMatchingServices`. Returns the resulting iterator over
/// every USB device. We deliberately do NOT add `idVendor` to the
/// matching dict here - on Apple Silicon the kernel translates the
/// legacy `IOUSBDevice` class name to `IOUSBHostDevice` via class
/// inheritance, but property-level filters don't apply across that
/// translation. Filtering by vendor id happens client-side in
/// [`device_info_from_service`]. This is also what `libusb` does on
/// modern macOS.
fn matching_iter() -> Option<IOObject> {
    // SAFETY: `IOServiceMatching` is safe to call with a static C
    // string; on success it returns a +1 retained dictionary which
    // `IOServiceGetMatchingServices` consumes for us.
    let matching = unsafe { IOServiceMatching(kIOUSBDeviceClassName) };
    if matching.is_null() {
        return None;
    }
    let mut iter: io_iterator_t = 0;
    // SAFETY: `matching` is consumed by `IOKit` (`CFRelease`d on
    // success and failure both), and `iter` is written by the kernel.
    let kr = unsafe {
        IOServiceGetMatchingServices(kIOMasterPortDefault, matching.cast_const(), &raw mut iter)
    };
    if kr != KERN_SUCCESS || iter == 0 {
        return None;
    }
    Some(IOObject(iter))
}

/// Pull the properties we care about off a single `IOUSBDevice` service
/// and assemble a [`crate::discovery::DeviceInfo`]. Returns `None` if a
/// required property is missing or the product id is not one we know.
fn device_info_from_service(service: io_service_t) -> Option<crate::discovery::DeviceInfo> {
    let vendor_id = read_u32_property(service, "idVendor")?;
    let product_id = read_u32_property(service, "idProduct")?;
    if vendor_id != u32::from(meet2::VENDOR_ID) {
        return None;
    }
    let product_type = match u16::try_from(product_id).ok()? {
        meet2::PRODUCT_ID_MEET2 => ProductType::Meet2,
        _ => return None,
    };
    let serial = read_string_property(service, "USB Serial Number").unwrap_or_default();
    let registry_id = registry_entry_id(service)?;
    Some(crate::discovery::DeviceInfo {
        vendor_id: u16::try_from(vendor_id).ok()?,
        product_id: u16::try_from(product_id).ok()?,
        product_type,
        serial,
        registry_id,
    })
}

/// Read a `CFNumber`-valued registry property and return it as `u32`.
fn read_u32_property(entry: io_registry_entry_t, key: &str) -> Option<u32> {
    let value = read_property(entry, key)?;
    let number: CFNumber = value.downcast::<CFNumber>()?;
    let v = number.to_i64()?;
    u32::try_from(v).ok()
}

/// Read a `CFString`-valued registry property and return it as a Rust string.
fn read_string_property(entry: io_registry_entry_t, key: &str) -> Option<String> {
    let value = read_property(entry, key)?;
    let s: CFString = value.downcast::<CFString>()?;
    Some(s.to_string())
}

/// Look up a single property on the registry entry and wrap it in a
/// `CFType` so the destructor releases it. `None` when the property is
/// absent.
fn read_property(entry: io_registry_entry_t, key: &str) -> Option<CFType> {
    let cf_key = CFString::new(key);
    // SAFETY: `entry` is a live registry entry, `cf_key` outlives the
    // call. The returned `CFTypeRef` obeys the create rule.
    let cf_ptr = unsafe {
        IORegistryEntryCreateCFProperty(entry, cf_key.as_concrete_TypeRef(), kCFAllocatorDefault, 0)
    };
    if cf_ptr.is_null() {
        return None;
    }
    // SAFETY: `cf_ptr` is a +1 CF reference; `wrap_under_create_rule`
    // transfers ownership to the `CFType` wrapper.
    Some(unsafe { CFType::wrap_under_create_rule(cf_ptr) })
}

/// Read the `IOKit` registry entry id for the service. This survives
/// re-enumeration and is what we use to re-find a device for opening.
fn registry_entry_id(entry: io_registry_entry_t) -> Option<u64> {
    let mut id: u64 = 0;
    // SAFETY: `entry` is a live registry entry, `id` is initialised.
    let kr = unsafe { IORegistryEntryGetRegistryEntryID(entry, &raw mut id) };
    if kr == KERN_SUCCESS {
        Some(id)
    } else {
        None
    }
}
