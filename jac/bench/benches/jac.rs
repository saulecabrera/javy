use anyhow::Result;
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use std::time::{Duration, Instant};
use wasmtime::*;
use wasmtime_wasi::WasiCtxBuilder;
use wasmtime_wasi::preview1::{self, WasiP1Ctx};

pub const PLUGIN_MODULE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/plugin.wasm"));
pub const JACRT_MODULE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/jacrt.wasm"));
pub const LINK_MODULE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/link.wat"));

pub fn interpreter(c: &mut Criterion) {
    c.bench_function("interpreter", move |b| {
        b.iter_custom(|iters| {
            let mut total_duration = Duration::ZERO;
            for _ in 0..iters {
                let (engine, mut linker, mut store) = prepare_execution_env().unwrap();
                let interpreted =
                    Module::new(&engine, include_bytes!("./interpreted.wasm")).unwrap();
                let now = Instant::now();
                let interpreted_instance = linker
                    .instantiate(store.as_context_mut(), &interpreted)
                    .unwrap();
                let start = interpreted_instance
                    .get_typed_func::<(), ()>(store.as_context_mut(), "_start")
                    .unwrap();
                start.call(store.as_context_mut(), ()).unwrap();
                total_duration += now.elapsed();
            }
            total_duration
        })
    });
}

pub fn compiler(c: &mut Criterion) {
    c.bench_function("compiler", move |b| {
        b.iter_custom(|iters| {
            let mut total_duration = Duration::ZERO;
            for _ in 0..iters {
                let (engine, mut linker, mut store) = prepare_execution_env().unwrap();
                let interpreted = Module::new(&engine, include_bytes!("./compiled.wasm")).unwrap();
                let now = Instant::now();
                let compiled_instance = linker
                    .instantiate(store.as_context_mut(), &interpreted)
                    .unwrap();
                let start = compiled_instance
                    .get_typed_func::<(), ()>(store.as_context_mut(), "_start")
                    .unwrap();
                start.call(store.as_context_mut(), ()).unwrap();
                total_duration += now.elapsed();
            }
            total_duration
        })
    });
}

fn prepare_execution_env() -> Result<(Engine, Linker<WasiP1Ctx>, Store<WasiP1Ctx>)> {
    let engine = Engine::default();
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
    // TODO: Parametrize or extract from the module's custom section.
    linker.instance(
        store.as_context_mut(),
        "javy-default-plugin-v2",
        plugin_instance,
    )?;

    Ok((engine, linker, store))
}

criterion_group!(benches, interpreter, compiler);
criterion_main!(benches);
