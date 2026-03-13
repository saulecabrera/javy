#![allow(dead_code)]
#![allow(unused)]

//! JAC - The Javy Ahead-of-Time Compiler.

use anyhow::Result;
use jac_translate::TranslationBuilder;
mod args;
mod builder;
mod compiler;
mod control;
mod crt;
mod frontend;
mod linkage;
mod stack;

use compiler::Compiler;
use linkage::Linkage;

pub fn compile(bytes: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut builder = TranslationBuilder::new();
    let translation = builder.translate(bytes)?;
    let (module, func_table_len) = Compiler::new(translation).compile()?;
    let linkage = Linkage::new(func_table_len).emit()?;
    Ok((module, linkage))
}
