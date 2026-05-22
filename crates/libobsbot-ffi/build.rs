// SPDX-License-Identifier: GPL-3.0-only
//! Build script for libobsbot-ffi.
//!
//! Regenerates `include/libobsbot.h` via cbindgen only when explicitly asked
//! (`LIBOBSBOT_GEN_HEADER=1`). Crates.io users and ordinary `cargo build`
//! invocations do not need a C toolchain - they get the committed header.

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=LIBOBSBOT_GEN_HEADER");
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=../../cbindgen.toml");

    if env::var_os("LIBOBSBOT_GEN_HEADER").is_none() {
        return;
    }

    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = crate_dir.parent().unwrap().parent().unwrap();
    let config_path = workspace_root.join("cbindgen.toml");
    let out_path = workspace_root.join("include").join("libobsbot.h");

    let config = cbindgen::Config::from_file(&config_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", config_path.display()));

    cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(config)
        .generate()
        .unwrap_or_else(|e| panic!("cbindgen generate: {e}"))
        .write_to_file(&out_path);
}
