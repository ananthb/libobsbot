// SPDX-License-Identifier: GPL-3.0-only
//! Smoke CLI for manual verification against a real OBSBOT camera.
//!
//! v0.0.0 only proves the API compiles; every operation reports
//! `Unsupported`. Real subcommands land starting at M3.

use libobsbot_core::{Devices, Error};

fn main() {
    let Ok(devices) = Devices::new() else {
        eprintln!("failed to start hot-plug watcher");
        std::process::exit(1);
    };

    let list = devices.list();
    if list.is_empty() {
        println!("no OBSBOT cameras detected");
    } else {
        for info in &list {
            println!(
                "{:?} {:04x}:{:04x} sn={}",
                info.product_type, info.vendor_id, info.product_id, info.serial
            );
        }
    }

    if let Some(info) = list.first() {
        match devices.open(info) {
            Ok(_d) => println!("opened {}", info.serial),
            Err(Error::Unsupported(why)) => println!("open: unsupported ({why})"),
            Err(e) => eprintln!("open: {e}"),
        }
    }
}
