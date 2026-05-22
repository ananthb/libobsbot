// SPDX-License-Identifier: GPL-3.0-only
//! Smoke CLI for manual verification against a real OBSBOT camera.
//!
//! Enumerates connected cameras, opens the first one, and reads a small set
//! of standard UVC controls (brightness, contrast, saturation) plus their
//! reported ranges. All of these route through the `uvcvideo` driver via
//! V4L2 ioctls on `/dev/videoN`; OBSBOT XU methods still return
//! `Unsupported` until per-method captures land.

use libobsbot_core::{AiMode, AutoFramingMode, Cadence, Devices, Error, Event, WdrMode};

#[allow(clippy::too_many_lines)]
fn main() {
    if std::env::var_os("RUST_LOG").is_some() {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .with_writer(std::io::stderr)
            .init();
    }
    let Ok(devices) = Devices::new() else {
        eprintln!("failed to start hot-plug watcher");
        std::process::exit(1);
    };

    // Hot-plug events from the watcher thread. Drain whatever the
    // initial scan emitted for cameras present at startup.
    let events = devices.events();
    std::thread::sleep(std::time::Duration::from_millis(200));
    println!("hot-plug events so far:");
    while let Ok(ev) = events.try_recv() {
        match ev {
            Event::DeviceAdded { serial } => println!("  + added  serial={serial:?}"),
            Event::DeviceRemoved { serial } => println!("  - removed serial={serial:?}"),
            Event::Status { serial, snapshot } => {
                println!("  ~ status serial={serial:?} {snapshot:?}");
            }
            _ => println!("  ? unknown event variant"),
        }
    }

    let list = devices.list();
    if list.is_empty() {
        println!("no OBSBOT cameras detected");
        return;
    }
    for info in &list {
        println!(
            "{:?} {:04x}:{:04x} sn={}",
            info.product_type, info.vendor_id, info.product_id, info.serial
        );
    }

    let info = list.first().unwrap();
    let device = match devices.open(info) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("open: {e}");
            std::process::exit(1);
        }
    };
    println!("opened {}", device.name());

    report("brightness", device.brightness(), device.brightness_range());
    report("contrast", device.contrast(), device.contrast_range());
    report("saturation", device.saturation(), device.saturation_range());

    print!("center pan/tilt... ");
    match device.set_pan_tilt(0.0, 0.0) {
        Ok(()) => println!("ok"),
        Err(e) => println!("{e}"),
    }

    print!("set WDR off (XU selector 0x06)... ");
    match device.set_wdr(WdrMode::Off) {
        Ok(()) => println!("ok"),
        Err(e) => println!("{e}"),
    }

    print!("set AI mode off (XU selector 0x06, control id 0x16)... ");
    match device.set_ai_mode(AiMode::None) {
        Ok(()) => println!("ok"),
        Err(e) => println!("{e}"),
    }

    print!("set auto-framing Group (XU selector 0x06, control id 0x0d)... ");
    match device.set_auto_framing(AutoFramingMode::Group) {
        Ok(()) => println!("ok"),
        Err(e) => println!("{e}"),
    }

    print!("set face-focus off (XU RPC, synthesised frame)... ");
    match device.set_face_focus(false) {
        Ok(()) => println!("ok"),
        Err(e) => println!("{e}"),
    }

    print!("firmware from camera (XU RPC, synthesised frame)... ");
    match device.firmware_from_camera() {
        Ok(s) => println!("{s}"),
        Err(e) => println!("{e}"),
    }

    print!("serial from camera (XU RPC, synthesised frame)... ");
    match device.serial_from_camera() {
        Ok(s) => println!("{s}"),
        Err(e) => println!("{e}"),
    }

    match device.pan_tilt() {
        Ok((p, t)) => println!("  pan/tilt = ({p:+.3}, {t:+.3})"),
        Err(e) => println!("  pan/tilt: {e}"),
    }
    match device.zoom() {
        Ok(z) => println!("  zoom = {z}"),
        Err(e) => println!("  zoom: {e}"),
    }
    match device.focus() {
        Ok(f) => println!("  focus = {f}"),
        Err(e) => println!("  focus: {e}"),
    }

    print!("status() snapshot... ");
    match device.status() {
        Ok(s) => println!(
            "fw={} sn={} b={} c={} s={} pt=({:+.3},{:+.3}) z={}",
            s.firmware, s.serial, s.brightness, s.contrast, s.saturation, s.pan, s.tilt, s.zoom
        ),
        Err(e) => println!("{e}"),
    }

    // The poller runs on Cadence::Slow (2.5 s) by default. Bump to Fast
    // so we see a couple of samples during the smoke run, then drain them.
    device.set_status_cadence(Cadence::Fast);
    std::thread::sleep(std::time::Duration::from_millis(150));
    println!("status poller samples (Cadence::Fast):");
    let mut shown = 0;
    while let Ok(ev) = events.try_recv() {
        if let Event::Status { serial, snapshot } = ev {
            println!(
                "  status serial={serial:?} brightness={} contrast={} saturation={}",
                snapshot.brightness, snapshot.contrast, snapshot.saturation
            );
            shown += 1;
            if shown >= 3 {
                break;
            }
        }
    }
}

fn report(
    name: &str,
    value: Result<i32, Error>,
    range: Result<core::ops::RangeInclusive<i32>, Error>,
) {
    match (value, range) {
        (Ok(v), Ok(r)) => println!("  {name} = {v}  (range {}..={})", r.start(), r.end()),
        (Ok(v), Err(e)) => println!("  {name} = {v}  (range error: {e})"),
        (Err(e), _) => println!("  {name}: {e}"),
    }
}
