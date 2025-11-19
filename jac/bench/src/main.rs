use anyhow::Result;
use std::{env, fs};
use wasmtime::*;
use wasmtime_wasi::WasiCtxBuilder;
use wasmtime_wasi::preview1::{self, WasiP1Ctx};

const FUEL: u64 = u64::MAX;
pub const PLUGIN_MODULE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/plugin.wasm"));
pub const JACRT_MODULE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/jacrt.wasm"));
pub const LINK_MODULE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/link.wat"));

fn main() -> Result<()> {
    let args = env::args().collect::<Vec<_>>();

    let interpreted_bytes = fs::read(&args[1])?;
    let compiled_bytes = fs::read(&args[2])?;

    let mut config = Config::new();
    config.consume_fuel(true);
    let engine = Engine::new(&config)?;
    let mut linker: Linker<WasiP1Ctx> = Linker::new(&engine);
    preview1::add_to_linker_sync(&mut linker, |t| t)?;
    let wasi_ctx = WasiCtxBuilder::new()
        .inherit_stdio()
        .inherit_env()
        .build_p1();
    let mut store = Store::new(&engine, wasi_ctx);

    let link_module = Module::new(&engine, LINK_MODULE)?;
    let link_instance = linker.instantiate(store.as_context_mut(), &link_module)?;
    linker.instance(store.as_context_mut(), "jacrt-link", link_instance)?;

    let jacrt_module = Module::new(&engine, JACRT_MODULE)?;
    let jacrt_instance = linker.instantiate(store.as_context_mut(), &jacrt_module)?;
    linker.instance(store.as_context_mut(), "jacrt", jacrt_instance)?;

    let plugin_module = Module::new(&engine, PLUGIN_MODULE)?;
    let plugin_instance = linker.instantiate(store.as_context_mut(), &plugin_module)?;
    linker.instance(
        store.as_context_mut(),
        "javy-default-plugin-v2",
        plugin_instance,
    )?;

    // Code execution

    let interpreted_module = Module::new(&engine, interpreted_bytes)?;
    let compiled_module = Module::new(&engine, compiled_bytes)?;
    let interpreted_instance = linker.instantiate(store.as_context_mut(), &interpreted_module)?;
    let compiled_instance = linker.instantiate(store.as_context_mut(), &compiled_module)?;

    store.set_fuel(FUEL)?;
    let f = interpreted_instance.get_typed_func::<(), ()>(store.as_context_mut(), "_start")?;
    f.call(store.as_context_mut(), ())?;
    println!(
        "Fuel consumed for interpreted module: {}",
        FUEL - store.get_fuel()?
    );

    store.set_fuel(FUEL)?;
    let f = compiled_instance.get_typed_func::<(), ()>(store.as_context_mut(), "_start")?;
    f.call(store.as_context_mut(), ())?;
    println!(
        "Fuel consumed for compiled module: {}",
        FUEL - store.get_fuel()?
    );

    Ok(())
}
