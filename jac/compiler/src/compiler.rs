use crate::builder::FunctionBuilder;
use anyhow::Result;
use jac_translate::{FunctionTranslation, Translation, quickpars::Opcode};
use waffle::Module;

/// QuickJS-bytecode-to-Wasm compiler.
pub(crate) struct Compiler<'data> {
    /// QuickJS bytecode in memory representation.
    translation: Translation<'data>,
    /// The resulting WebAssembly module.
    result: Module<'data>,
}

impl<'data> Compiler<'data> {
    /// Create a new compiler from the translated QuickJS bytecode.
    pub fn new(translation: Translation<'data>) -> Self {
        Self {
            translation,
            result: Module::empty(),
        }
    }

    // TODO: Before starting each function compilation, create the
    //       known module prelude.  e.g., we could start by adding the
    //       functions table with the known number of functions
    //       (`translation.module.functions.len()`)
    /// Perform compilation into Wasm bytes.
    pub fn compile(mut self) -> Result<Vec<u8>> {
        for func in &self.translation.module.functions {
            FunctionBuilder::new(&mut self.result, func).build(&self.translation)?;
        }
        self.result.to_wasm_bytes()
    }
}
