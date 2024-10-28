mod bytecode;
mod codegen;
mod commands;
mod js;
mod option;
mod wit;

use crate::commands::{Cli, Command, EmitProviderCommandOpts};
use anyhow::Result;
use clap::Parser;
use codegen::{CodeGenBuilder, DynamicGenerator, StaticGenerator};
use commands::{CodegenOptionGroup, JsOptionGroup};
use js::JS;
use std::fs;
use std::fs::File;
use std::io::Write;

fn main() -> Result<()> {
    let args = Cli::parse();

    match &args.command {
        Command::EmitProvider(opts) => emit_provider(opts),
        Command::Compile(opts) => {
            let js = JS::from_file(&opts.input)?;
            let bytes = js.compile()?;
            jacc::compile(&bytes)?;
            Ok(())
        }
        Command::Build(opts) => {
            let js = JS::from_file(&opts.input)?;
            let codegen: CodegenOptionGroup = opts.codegen.clone().try_into()?;
            let mut builder = CodeGenBuilder::new();
            builder
                .wit_opts(codegen.wit)
                .source_compression(codegen.source_compression)
                .provider_version("3");

            let js_opts: JsOptionGroup = opts.js.clone().into();
            let mut gen = if codegen.dynamic {
                builder.build::<DynamicGenerator>(js_opts.into())?
            } else {
                builder.build::<StaticGenerator>(js_opts.into())?
            };

            let wasm = gen.generate(&js)?;

            fs::write(&opts.output, wasm)?;
            Ok(())
        }
    }
}

fn emit_provider(opts: &EmitProviderCommandOpts) -> Result<()> {
    let mut file: Box<dyn Write> = match opts.out.as_ref() {
        Some(path) => Box::new(File::create(path)?),
        _ => Box::new(std::io::stdout()),
    };
    file.write_all(bytecode::QUICKJS_PROVIDER_MODULE)?;
    Ok(())
}
