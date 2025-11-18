//! Function Builder.
use anyhow::Result;
use jac_translate::{FunctionTranslation, quickpars::BinaryReader};
use std::collections::{HashMap, HashSet};

/// An IR builder.
pub(crate) struct FunctionBuilder<'a, 'data> {
    /// The QuickJS bytecode function translation.
    translation: &'a FunctionTranslation<'data>,
}

impl<'a, 'data> FunctionBuilder<'a, 'data> {
    pub fn new(translation: &'a FunctionTranslation<'data>) -> Self {
        Self { translation }
    }

    pub fn build(mut self) -> Result<()> {
        todo!()
    }

    fn handle_operator(&mut self, _reader: &mut BinaryReader<'data>) -> Result<()> {
        Ok(())
    }
}
