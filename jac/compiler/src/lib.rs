#![allow(dead_code)]
#![allow(unused)]

//! JAC - The Javy Ahead-of-Time Compiler.

use anyhow::Result;
use jac_translate::TranslationBuilder;
mod builder;
mod compiler;
mod crt;
mod frontend;
mod stack;

use compiler::Compiler;

pub fn compile(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut builder = TranslationBuilder::new();
    let translation = builder.translate(bytes)?;
    Compiler::new(translation)
	.compile()
}
