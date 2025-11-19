use anyhow::{Result, bail};
use std::path::PathBuf;
use std::{env, fs};

fn main() -> Result<()> {
    let cargo_manifest_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let root = cargo_manifest_path.parent().unwrap().parent().unwrap();
    let linkage = root.join("crates/jacrt/link.wat");
    let jacrt = root.join("target/wasm32-wasip1/release/jacrt.wasm");
    let plugin = root.join("target/wasm32-wasip1/release/plugin.wasm");

    if !linkage.exists() || !jacrt.exists() || !plugin.exists() {
        bail!("Missing wasm artifacts to build compiler benchmarks");
    }

    println!("cargo:rerun-if-changed={}", plugin.to_str().unwrap());
    println!("cargo:rerun-if-changed={}", jacrt.to_str().unwrap());
    println!("cargo:rerun-if-changed={}", linkage.to_str().unwrap());
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let initialized_plugin = javy_plugin_processing::initialize_plugin(&fs::read(&plugin)?)?;
    let initialized_jacrt = javy_plugin_processing::initialize_plugin(&fs::read(&jacrt)?)?;

    fs::write(&out_dir.join("jacrt.wasm"), &initialized_jacrt)?;
    fs::write(&out_dir.join("plugin.wasm"), &initialized_plugin)?;
    fs::copy(&linkage, &out_dir.join("link.wat"))?;

    Ok(())
}
