use crate::builder::FunctionBuilder;
use anyhow::Result;
use jac_translate::{FunctionTranslation, Translation, quickpars::Opcode};

/// QuickJS-bytecode-to-Wasm compiler.
pub(crate) struct Compiler<'data> {
    /// QuickJS bytecode in memory representation.
    translation: Translation<'data>,
}

impl<'data> Compiler<'data> {
    /// Create a new compiler from the translated QuickJS bytecode.
    pub fn new(translation: Translation<'data>) -> Self {
        Self {
            translation,
        }
    }

    /// Perform compilation into Wasm bytes.
    pub fn compile(&mut self) -> Result<Vec<u8>> {
	todo!()
    }
}
