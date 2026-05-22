// SPDX-License-Identifier: GPL-3.0-only
//! Linux uvcvideo coexistence: V4L2 standard controls + UVC XU ioctls.
//!
//! On Linux the kernel rejects raw usbfs control transfers (`SUBMITURB`)
//! addressed to an interface that another driver has claimed — i.e. always,
//! for the `VideoControl` interface, because `uvcvideo` claims it the moment
//! the camera is plugged in. The supported coexistence path is to open the
//! camera's `/dev/videoN` node and use:
//!
//! - **V4L2 standard control ioctls** (`VIDIOC_G_CTRL`, `VIDIOC_S_CTRL`,
//!   `VIDIOC_QUERYCTRL`) for `CameraTerminal` (entity 1) and
//!   `ProcessingUnit` (entity 3). `uvcvideo` translates V4L2 control IDs
//!   into UVC class-specific transfers.
//! - **`UVCIOC_CTRL_QUERY`** for the OBSBOT vendor `ExtensionUnit`
//!   (entity 2). Per UVC controls have to be registered via
//!   `UVCIOC_CTRL_MAP` first (deferred until per-XU pcaps land).
//!
//! See `Documentation/userspace-api/media/v4l/uvc.rst` and
//! `include/uapi/linux/uvcvideo.h` in the Linux kernel tree.

#![cfg(target_os = "linux")]

use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::path::PathBuf;

use crate::uvc::UvcGet;
use crate::{Error, Result};

// ---- UVC XU ioctl --------------------------------------------------------

/// `_IOWR('u', 0x21, struct uvc_xu_control_query)` (16-byte struct on 64-bit).
const UVCIOC_CTRL_QUERY: libc::c_ulong = 0xc010_7521;

#[repr(C)]
struct UvcXuControlQuery {
    unit: u8,
    selector: u8,
    query: u8,
    size: u16,
    data: *mut u8,
}

const _: () = assert!(core::mem::size_of::<UvcXuControlQuery>() == 16);

/// Issue `UVCIOC_CTRL_QUERY` against the open `/dev/videoN` handle.
pub(super) fn xu_query(
    fd: &File,
    unit: u8,
    selector: u8,
    query: u8,
    data: &mut [u8],
) -> Result<usize> {
    let size = u16::try_from(data.len())
        .map_err(|_| Error::Usb(format!("uvcvideo: buffer too large ({} bytes)", data.len())))?;
    let mut q = UvcXuControlQuery {
        unit,
        selector,
        query,
        size,
        data: data.as_mut_ptr(),
    };
    // SAFETY: `q` is initialised and `data` lives for the call's duration.
    let rc = unsafe { libc::ioctl(fd.as_raw_fd(), UVCIOC_CTRL_QUERY, &mut q) };
    if rc < 0 {
        return Err(Error::Usb(format!(
            "UVCIOC_CTRL_QUERY unit={unit} sel={selector:#04x} query={query:#04x}: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(data.len())
}

// ---- V4L2 standard controls ----------------------------------------------

const V4L2_CID_BASE: u32 = 0x0098_0900;
const V4L2_CID_CAMERA_CLASS_BASE: u32 = 0x009a_0900;

const V4L2_CID_BRIGHTNESS: u32 = V4L2_CID_BASE;
const V4L2_CID_CONTRAST: u32 = V4L2_CID_BASE + 1;
const V4L2_CID_SATURATION: u32 = V4L2_CID_BASE + 2;
const V4L2_CID_AUTO_WHITE_BALANCE: u32 = V4L2_CID_BASE + 12;
const V4L2_CID_WHITE_BALANCE_TEMPERATURE: u32 = V4L2_CID_BASE + 26;
const V4L2_CID_PAN_ABSOLUTE: u32 = V4L2_CID_CAMERA_CLASS_BASE + 8;
const V4L2_CID_TILT_ABSOLUTE: u32 = V4L2_CID_CAMERA_CLASS_BASE + 9;
const V4L2_CID_FOCUS_ABSOLUTE: u32 = V4L2_CID_CAMERA_CLASS_BASE + 10;
const V4L2_CID_ZOOM_ABSOLUTE: u32 = V4L2_CID_CAMERA_CLASS_BASE + 13;

/// `_IOWR('V', 27, struct v4l2_control)` (8-byte struct).
const VIDIOC_G_CTRL: libc::c_ulong = 0xc008_561b;
/// `_IOWR('V', 28, struct v4l2_control)` (8-byte struct).
const VIDIOC_S_CTRL: libc::c_ulong = 0xc008_561c;
/// `_IOWR('V', 36, struct v4l2_queryctrl)` (68-byte struct).
const VIDIOC_QUERYCTRL: libc::c_ulong = 0xc044_5624;

#[repr(C)]
struct V4l2Control {
    id: u32,
    value: i32,
}
const _: () = assert!(core::mem::size_of::<V4l2Control>() == 8);

#[repr(C)]
struct V4l2Queryctrl {
    id: u32,
    type_: u32,
    name: [u8; 32],
    minimum: i32,
    maximum: i32,
    step: i32,
    default_value: i32,
    flags: u32,
    reserved: [u32; 2],
}
const _: () = assert!(core::mem::size_of::<V4l2Queryctrl>() == 68);

/// Map a UVC `(entity, selector)` pair to a V4L2 control id, where
/// `uvcvideo` exposes the control through the standard V4L2 API.
///
/// `CT_PANTILT_ABSOLUTE_CONTROL` is handled by [`v4l2_set`] / [`v4l2_get`]
/// directly — UVC packs pan and tilt into one 8-byte control, but V4L2
/// splits them into [`V4L2_CID_PAN_ABSOLUTE`] and [`V4L2_CID_TILT_ABSOLUTE`].
fn cid_for(entity: u8, selector: u8) -> Option<u32> {
    match (entity, selector) {
        // Processing Unit
        (3, 0x02) => Some(V4L2_CID_BRIGHTNESS),
        (3, 0x03) => Some(V4L2_CID_CONTRAST),
        (3, 0x07) => Some(V4L2_CID_SATURATION),
        (3, 0x0a) => Some(V4L2_CID_WHITE_BALANCE_TEMPERATURE),
        (3, 0x0b) => Some(V4L2_CID_AUTO_WHITE_BALANCE),
        // Camera Terminal
        (1, 0x06) => Some(V4L2_CID_FOCUS_ABSOLUTE),
        (1, 0x0b) => Some(V4L2_CID_ZOOM_ABSOLUTE),
        _ => None,
    }
}

/// True iff `(entity, selector)` is `CT_PANTILT_ABSOLUTE_CONTROL`.
fn is_pantilt(entity: u8, selector: u8) -> bool {
    entity == 1 && selector == 0x0d
}

/// True when the V4L2 control's 2-byte UVC payload is a signed `i16`.
/// In UVC 1.5 PU, only `BRIGHTNESS` and `HUE` are signed (HUE not yet
/// exposed in our `cid_for` table).
fn is_signed_2byte(cid: u32) -> bool {
    cid == V4L2_CID_BRIGHTNESS
}

/// Dispatch a `(entity, selector)` SET to V4L2 when there's a mapping.
/// Returns `None` if there is no V4L2 cid for this pair (caller should
/// fall back to `nusb` or the XU path).
pub(super) fn v4l2_set(fd: &File, entity: u8, selector: u8, payload: &[u8]) -> Option<Result<()>> {
    if is_pantilt(entity, selector) {
        return Some(pantilt_set(fd, payload));
    }
    let cid = cid_for(entity, selector)?;
    Some(v4l2_s_ctrl(fd, cid, payload_to_i32(cid, payload)))
}

/// Dispatch a `(entity, selector)` GET to V4L2 when there's a mapping.
/// Writes the returned value into `out` as little-endian bytes, sized to
/// `out.len()`.
pub(super) fn v4l2_get(
    fd: &File,
    req: UvcGet,
    entity: u8,
    selector: u8,
    out: &mut [u8],
) -> Option<Result<usize>> {
    if is_pantilt(entity, selector) {
        return Some(pantilt_get(fd, req, out));
    }
    let cid = cid_for(entity, selector)?;
    let result = match req {
        UvcGet::Cur => v4l2_g_ctrl(fd, cid).map(i32::to_le_bytes),
        UvcGet::Min => v4l2_queryctrl(fd, cid).map(|q| q.minimum.to_le_bytes()),
        UvcGet::Max => v4l2_queryctrl(fd, cid).map(|q| q.maximum.to_le_bytes()),
    };
    Some(result.map(|bytes| {
        let n = bytes.len().min(out.len());
        out[..n].copy_from_slice(&bytes[..n]);
        n
    }))
}

/// Split UVC's 8-byte `(i32 pan, i32 tilt)` payload across the two V4L2
/// `PAN_ABSOLUTE` and `TILT_ABSOLUTE` controls.
fn pantilt_set(fd: &File, payload: &[u8]) -> Result<()> {
    if payload.len() != 8 {
        return Err(Error::Usb(format!(
            "CT_PANTILT_ABSOLUTE expects 8 bytes, got {}",
            payload.len()
        )));
    }
    let pan = i32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let tilt = i32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
    v4l2_s_ctrl(fd, V4L2_CID_PAN_ABSOLUTE, pan)?;
    v4l2_s_ctrl(fd, V4L2_CID_TILT_ABSOLUTE, tilt)
}

/// Combine the two V4L2 PAN/TILT controls back into UVC's packed 8-byte
/// `(i32 pan, i32 tilt)` representation. For `Min`/`Max`, each axis's own
/// range is returned packed together.
fn pantilt_get(fd: &File, req: UvcGet, out: &mut [u8]) -> Result<usize> {
    if out.len() < 8 {
        return Err(Error::Usb(format!(
            "CT_PANTILT_ABSOLUTE response expects 8 bytes, got {}",
            out.len()
        )));
    }
    let (pan, tilt) = match req {
        UvcGet::Cur => (
            v4l2_g_ctrl(fd, V4L2_CID_PAN_ABSOLUTE)?,
            v4l2_g_ctrl(fd, V4L2_CID_TILT_ABSOLUTE)?,
        ),
        UvcGet::Min => (
            v4l2_queryctrl(fd, V4L2_CID_PAN_ABSOLUTE)?.minimum,
            v4l2_queryctrl(fd, V4L2_CID_TILT_ABSOLUTE)?.minimum,
        ),
        UvcGet::Max => (
            v4l2_queryctrl(fd, V4L2_CID_PAN_ABSOLUTE)?.maximum,
            v4l2_queryctrl(fd, V4L2_CID_TILT_ABSOLUTE)?.maximum,
        ),
    };
    out[..4].copy_from_slice(&pan.to_le_bytes());
    out[4..8].copy_from_slice(&tilt.to_le_bytes());
    Ok(8)
}

fn v4l2_g_ctrl(fd: &File, cid: u32) -> Result<i32> {
    let mut c = V4l2Control { id: cid, value: 0 };
    // SAFETY: `c` is initialised; the kernel writes `value`.
    let rc = unsafe { libc::ioctl(fd.as_raw_fd(), VIDIOC_G_CTRL, &mut c) };
    if rc < 0 {
        return Err(Error::Usb(format!(
            "VIDIOC_G_CTRL cid={cid:#x}: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(c.value)
}

fn v4l2_s_ctrl(fd: &File, cid: u32, value: i32) -> Result<()> {
    let mut c = V4l2Control { id: cid, value };
    // SAFETY: `c` is initialised; the kernel reads `value`.
    let rc = unsafe { libc::ioctl(fd.as_raw_fd(), VIDIOC_S_CTRL, &mut c) };
    if rc < 0 {
        return Err(Error::Usb(format!(
            "VIDIOC_S_CTRL cid={cid:#x} val={value}: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(())
}

fn v4l2_queryctrl(fd: &File, cid: u32) -> Result<V4l2Queryctrl> {
    let mut q = V4l2Queryctrl {
        id: cid,
        type_: 0,
        name: [0; 32],
        minimum: 0,
        maximum: 0,
        step: 0,
        default_value: 0,
        flags: 0,
        reserved: [0; 2],
    };
    // SAFETY: `q` is initialised; the kernel fills in the remaining fields.
    let rc = unsafe { libc::ioctl(fd.as_raw_fd(), VIDIOC_QUERYCTRL, &mut q) };
    if rc < 0 {
        return Err(Error::Usb(format!(
            "VIDIOC_QUERYCTRL cid={cid:#x}: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(q)
}

fn payload_to_i32(cid: u32, payload: &[u8]) -> i32 {
    match payload.len() {
        1 => i32::from(payload[0]),
        2 => {
            let bytes = [payload[0], payload[1]];
            if is_signed_2byte(cid) {
                i32::from(i16::from_le_bytes(bytes))
            } else {
                i32::from(u16::from_le_bytes(bytes))
            }
        }
        4 => i32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]),
        _ => 0,
    }
}

// ---- /dev/videoN discovery -----------------------------------------------

/// Find and open the `/dev/videoN` belonging to the given USB device.
///
/// Walks `/sys/class/video4linux/` and matches the parent USB device's
/// `busnum` + `devnum` against the values from `nusb`. Multiple v4l2 nodes
/// for the same UVC device are common (one per streaming interface); the
/// lowest-numbered one wins for determinism, and all of them are equally
/// valid handles for the v4l2 / UVC ioctls.
pub(super) fn open_for(usb_busnum: u8, usb_devnum: u8) -> Result<File> {
    let dir = std::fs::read_dir("/sys/class/video4linux")
        .map_err(|e| Error::Usb(format!("read /sys/class/video4linux: {e}")))?;
    let mut matches: Vec<std::ffi::OsString> = Vec::new();
    for entry in dir {
        let entry = entry.map_err(|e| Error::Usb(format!("readdir: {e}")))?;
        let device_link = entry.path().join("device");
        let Ok(iface_dir) = std::fs::canonicalize(&device_link) else {
            continue;
        };
        let Some(dev_dir) = iface_dir.parent() else {
            continue;
        };
        let (Ok(b), Ok(d)) = (
            read_u8(&dev_dir.join("busnum")),
            read_u8(&dev_dir.join("devnum")),
        ) else {
            continue;
        };
        if b == usb_busnum && d == usb_devnum {
            matches.push(entry.file_name());
        }
    }
    matches.sort();
    let Some(name) = matches.into_iter().next() else {
        return Err(Error::NotFound);
    };
    let path = PathBuf::from("/dev").join(&name);
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|e| Error::Usb(format!("open {}: {e}", path.display())))
}

fn read_u8(path: &std::path::Path) -> Result<u8> {
    let s = std::fs::read_to_string(path).map_err(|e| Error::Usb(e.to_string()))?;
    s.trim()
        .parse()
        .map_err(|_| Error::Usb(format!("parse u8 from {}", path.display())))
}
