//! Build script: emit `include/libc12n_core.h` from the `extern "C"` FFI
//! surface using cbindgen.
//!
//! The generated header is committed to the repository so that downstream
//! consumers (notably c12n-php via `FFI::cdef`) can ship it alongside the
//! cdylib without requiring cbindgen to be installed.

use std::env;
use std::path::PathBuf;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR must be set by cargo");
    let crate_dir = PathBuf::from(crate_dir);

    let include_dir = crate_dir.join("include");
    std::fs::create_dir_all(&include_dir)
        .expect("failed to create include/ directory");
    let out_path = include_dir.join("libc12n_core.h");

    let config_path = crate_dir.join("cbindgen.toml");
    let config = cbindgen::Config::from_file(&config_path)
        .expect("failed to load cbindgen.toml");

    // Re-run only when FFI surface or config changes.
    println!("cargo:rerun-if-changed=src/ffi.rs");
    println!("cargo:rerun-if-changed=cbindgen.toml");
    println!("cargo:rerun-if-changed=build.rs");

    cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(config)
        .generate()
        .expect("cbindgen failed to generate bindings")
        .write_to_file(out_path);
}
