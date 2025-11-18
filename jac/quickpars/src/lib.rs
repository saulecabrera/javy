//! QuickJS Bytecode Parser written in Rust.

use core::str;

use anyhow::{Context, Result, anyhow, ensure};

pub mod atom;
pub use atom::*;
pub mod bc;
pub use bc::*;
pub mod consts;
pub use consts::*;
pub mod op;
pub use op::*;
pub mod readers;
pub use readers::*;
pub mod sections;
pub use sections::*;

macro_rules! entity {
    ($name:ident) => {
        #[derive(Copy, Clone, Debug, Eq, PartialEq)]
        pub struct $name(u32);

        impl Default for $name {
            fn default() -> Self {
                // Reserved value.
                Self(u32::MAX)
            }
        }

        impl $name {
            /// Constructs a function entity from a given u32.
            pub fn from_u32(val: u32) -> Self {
                Self(val)
            }

            /// Returns the entity representation as u32.
            pub fn as_u32(&self) -> u32 {
                self.0
            }
        }
    };
}

entity!(AtomIndex);
entity!(LocalIndex);
entity!(ClosureVarIndex);
entity!(FuncIndex);
entity!(ConstantPoolIndex);

/// Known payload in the bytecode.
#[derive(Debug, Clone)]
pub enum Payload<'a> {
    Version(u8),
    Header(HeaderSection),
    ModuleHeader(ModuleSectionHeader),
    FunctionHeader((FuncIndex, FunctionSectionHeader)),
    FunctionLocals((FuncIndex, Vec<FunctionLocal>)),
    FunctionClosureVars((FuncIndex, Vec<FunctionClosureVar>)),
    FunctionDebugInfo((FuncIndex, DebugInfo<'a>)),
    FunctionOperators((FuncIndex, BinaryReader<'a>)),
    End,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum ParserState {
    /// Bytecode version.
    Version,
    /// Bytecode header.
    Header,
    /// Tags present in the code section.
    Tags,
    /// Locals in the code section.
    FunctionLocals,
    /// Closure variable information in the code section.
    FunctionClosureVars,
    /// Function operators.
    FunctionOperators,
    /// Debug information.
    Debug,
    /// End of bytecode.
    End,
}

/// Metadata about the current function.
#[derive(Debug, Clone)]
struct FuncMeta {
    /// The bytecode size in bytes.
    bytecode_len: u32,
    /// The number of locals.
    local_count: u32,
    /// The number of closure variables referenced by the function.
    closure_var_count: u32,
    // This is not used, yet.
    #[allow(dead_code)]
    /// The number of elements in the constant pool of the current function.
    constant_pool_size: u32,
    /// Whether the function encodes debug information.
    debug: bool,
}

/// A QuickJS bytecode parser.
#[derive(Debug, Clone)]
pub struct Parser {
    /// The state of the parser.
    state: ParserState,
    /// The current position of the parser.
    offset: usize,
    /// The function definition depth, relative
    /// to the size of the constant pool.
    depth: u32,
    /// Is the parser done.
    done: bool,
    /// Metadata stack about the current function.
    meta: Vec<FuncMeta>,
}

impl Parser {
    /// Create a new [Parser].
    pub fn new() -> Self {
        Self {
            state: ParserState::Version,
            offset: 0,
            done: false,
            meta: vec![],
            depth: 0,
        }
    }
}

impl Parser {
    /// Parse the entire bytecode buffer.
    pub fn parse_buffer(self, data: &[u8]) -> impl Iterator<Item = Result<Payload<'_>>> {
        let mut parser = self;
        std::iter::from_fn(move || {
            if parser.done {
                return None;
            }
            Some(parser.parse(data))
        })
    }

    /// Get the current depth of the parsing state,
    /// which maps to the number of functions.
    fn depth(&self) -> u32 {
        u32::try_from(self.meta.len() - 1).unwrap()
    }

    /// Intermeidate parsing helper.
    fn parse<'a>(&mut self, data: &'a [u8]) -> Result<Payload<'a>> {
        // Every time `parse` is called, make sure to update the view of data
        // that we're parsing via `&data[self.offset...]`
        let mut reader = BinaryReader::new(&data[self.offset..]);
        match self.parse_with(&mut reader) {
            Ok(payload) => {
                self.offset += reader.offset;
                if self.offset >= data.len() {
                    self.done = true;
                }
                Ok(payload)
            }
            Err(err) => {
                self.done = true;
                Err(err).with_context(|| {
                    format!(
                        "Failed to parse bytecode at offset: {} and state: {:?}",
                        self.offset + reader.offset,
                        self.state,
                    )
                })
            }
        }
    }

    /// Performs binary parsing with the provided binary reader.
    fn parse_with<'a: 'b, 'b>(&mut self, reader: &'b mut BinaryReader<'a>) -> Result<Payload<'a>> {
        use Payload::*;

        match self.state {
            ParserState::Version => reader
                .read_u8()
                .and_then(validate_version)
                .map(Version)
                .map(|v| {
                    self.state = ParserState::Header;
                    v
                }),

            ParserState::Header => self.parse_header(reader),
            ParserState::Tags => reader
                .read_u8()
                .and_then(Tag::map_byte)
                .and_then(|tag| self.parse_tag(tag, reader)),
            ParserState::FunctionLocals => self.parse_local_section(reader),
            ParserState::FunctionClosureVars => self.parse_closure_var_section(reader),
            ParserState::FunctionOperators => self.parse_operators_section(reader),
            ParserState::Debug => self.parse_debug_section(reader),
            ParserState::End => {
                self.done = true;
                Ok(End)
            }
        }
    }

    /// Parse a QuickJS bytecode tag, dispatching to the right parsing function.
    fn parse_tag<'a: 'b, 'b>(
        &mut self,
        tag: Tag,
        reader: &'b mut BinaryReader<'a>,
    ) -> Result<Payload<'a>> {
        ensure!(
            self.state == ParserState::Tags,
            format!(
                "Expected parser state: {:?}, got: {:?}",
                ParserState::Tags,
                self.state
            ),
        );
        let payload = match tag {
            Tag::Module => self.parse_module_header(reader),
            Tag::FunctionBytecode => {
                let flags = reader.read_u16()?;
                // JS mode.
                // Are we in `strict` mode?
                reader.read_u8()?;
                // Function name.
                let name_index = reader.read_atom()?;
                // Arg count.
                let arg_count = reader.read_leb128()?;
                let var_count = reader.read_leb128()?;
                let defined_arg_count = reader.read_leb128()?;
                let stack_size = reader.read_leb128()?;
                let closure_var_count = reader.read_leb128()?;
                let constant_pool_size = reader.read_leb128()?;
                let bytecode_len = reader.read_leb128()?;
                let local_count = reader.read_leb128()?;
                let debug = flag(flags as u32, 9, 1);

                self.meta.push(FuncMeta {
                    local_count,
                    bytecode_len,
                    debug: debug != 0,
                    closure_var_count,
                    constant_pool_size,
                });

                self.state = ParserState::FunctionLocals;

                Ok(Payload::FunctionHeader((
                    FuncIndex::from_u32(self.depth),
                    FunctionSectionHeader {
                        flags,
                        name_index: AtomIndex::from_u32(name_index),
                        arg_count,
                        var_count,
                        defined_arg_count,
                        stack_size,
                        closure_var_count,
                        constant_pool_size,
                        bytecode_len,
                        local_count,
                    },
                )))
            }
            x => Err(anyhow!("Unsupported {x:?}")),
        };
        if reader.done() {
            self.state = ParserState::End;
        }
        payload
    }

    /// Parse the function's local section.
    fn parse_local_section<'a: 'b, 'b>(
        &mut self,
        reader: &'b mut BinaryReader<'a>,
    ) -> Result<Payload<'a>> {
        ensure!(
            self.state == ParserState::FunctionLocals,
            "Incorrect parser state, expected `FunctionLocals`"
        );
        ensure!(
            self.meta.len() > 0,
            "Expected function metadata in parser when parsing locals"
        );

        let local_count = self.meta.last().as_ref().unwrap().local_count;
        let mut locals = vec![];
        for _ in 0..local_count {
            locals.push(FunctionLocal {
                name_index: AtomIndex::from_u32(reader.read_atom()?),
                scope_level: reader.read_leb128()?,
                scope_next: reader.read_leb128()?,
                flags: reader.read_u8()?,
            });
        }

        self.state = ParserState::FunctionClosureVars;

        Ok(Payload::FunctionLocals((
            FuncIndex::from_u32(self.depth()),
            locals,
        )))
    }

    fn parse_closure_var_section<'a: 'b, 'b>(
        &mut self,
        reader: &'b mut BinaryReader<'a>,
    ) -> Result<Payload<'a>> {
        ensure!(
            self.state == ParserState::FunctionClosureVars,
            "Incorrect parser state, expected `FunctionClosureVars`"
        );
        ensure!(
            self.meta.len() > 0,
            "Expected function metadata in parser when parsing locals"
        );

        let closure_var_count = self.meta.last().as_ref().unwrap().closure_var_count;
        let mut closure_vars = vec![];
        for _ in 0..closure_var_count {
            let name_index = AtomIndex::from_u32(reader.read_atom()?);
            let index = reader.read_leb128()?;
            let flags = reader.read_u8()?;

            let is_local = bc::flag(flags as u32, 0, 1);
            let is_arg = bc::flag(flags as u32, 1, 1);
            let is_const = bc::flag(flags as u32, 2, 1);
            let is_lexical = bc::flag(flags as u32, 3, 1);
            let var_kind = bc::flag(flags as u32, 4, 4);

            closure_vars.push(FunctionClosureVar {
                name_index,
                index,
                is_local: is_local != 0,
                is_arg: is_arg != 0,
                is_const: is_const != 0,
                is_lexical: is_lexical != 0,
                var_kind: u8::try_from(var_kind).unwrap(),
            });
        }

        let result = Ok(Payload::FunctionClosureVars((
            FuncIndex::from_u32(self.depth()),
            closure_vars,
        )));
        if self.meta.last().as_ref().unwrap().constant_pool_size > 0 {
            self.state = ParserState::Tags;
        } else {
            self.state = ParserState::FunctionOperators;
        }

        result
    }

    fn parse_operators_section<'a: 'b, 'b>(
        &mut self,
        reader: &'b mut BinaryReader<'a>,
    ) -> Result<Payload<'a>> {
        ensure!(
            self.meta.len() > 0,
            format!(
                "Expected parser meta to be present with {:?}",
                ParserState::FunctionOperators
            ),
        );

        let len = self.meta.last().as_ref().unwrap().bytecode_len as usize;
        let p = Ok(Payload::FunctionOperators((
            FuncIndex::from_u32(self.depth()),
            slice(reader, len)?,
        )));

        if self.meta.last().as_ref().unwrap().debug {
            self.state = ParserState::Debug;
        } else {
            self.meta.pop();
            if self.meta.is_empty() {
                self.state = ParserState::End;
            } else {
                self.state = ParserState::FunctionOperators;
            }
        }

        p
    }

    /// Parse a function's debug section.
    fn parse_debug_section<'a: 'b, 'b>(
        &mut self,
        reader: &'b mut BinaryReader<'a>,
    ) -> Result<Payload<'a>> {
        ensure!(
            self.state == ParserState::Debug,
            format!(
                "Expected parser state {:?}, got {:?}",
                ParserState::Debug,
                self.state
            )
        );
        let filename = reader.read_atom()?;
        let lineno = reader.read_leb128()?;
        let colno = reader.read_leb128()?;
        let pc2line_len = reader.read_leb128()?;

        let pc2line_reader = if pc2line_len > 0 {
            Some(slice(reader, pc2line_len as usize)?)
        } else {
            None
        };

        let source_len = reader.read_leb128()?;

        let source_reader = if source_len > 0 {
            Some(slice(reader, source_len as usize)?)
        } else {
            None
        };

        let result = Ok(Payload::FunctionDebugInfo((
            FuncIndex::from_u32(self.depth),
            DebugInfo::new(filename, lineno, colno, pc2line_reader, source_reader),
        )));

        self.meta.pop();
        if self.meta.is_empty() {
            self.state = ParserState::End;
        } else {
            self.state = ParserState::FunctionOperators;
        }

        result
    }

    /// Parses the bytecode header.
    fn parse_header<'a: 'b, 'b>(
        &mut self,
        reader: &'b mut BinaryReader<'a>,
    ) -> Result<Payload<'a>> {
        ensure!(
            self.state == ParserState::Header,
            format!(
                "Expected parser state: {:?}, got: {:?}",
                ParserState::Header,
                self.state
            )
        );
        let atom_count = reader.read_leb128()?;
        let mut atoms = ATOM_NAMES
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        for _ in 0..atom_count {
            let ty = reader.read_u8()?;
            if ty == 0 {
                let _ = reader.read_u32()?;
            } else {
                atoms.push(str::from_utf8(read_str_bytes(reader)?)?.to_string());
            }
        }

        self.state = ParserState::Tags;

        Ok(Payload::Header(HeaderSection::new(atom_count, atoms)))
    }

    // Parse the module header.
    fn parse_module_header<'a: 'b, 'b>(
        &mut self,
        reader: &'b mut BinaryReader<'a>,
    ) -> Result<Payload<'a>> {
        // The entity of the atom array containing the module name.
        let name_entity = reader.read_atom()?;
        self.meta = vec![];

        let mut req_modules = vec![];
        let mut exports = vec![];
        let mut star_exports = vec![];
        let mut imports = vec![];
        let has_tla = reader
            // Req module entries count.
            .read_leb128()
            // Each dependency.
            .and_then(|deps| {
                (0..deps).try_for_each(|_| {
                    req_modules.push(reader.read_atom()?);
                    Ok(())
                })
            })
            // Exports count.
            .and_then(|_| reader.read_leb128())
            // Each export.
            .and_then(|count| {
                (0..count).try_for_each(|_| {
                    let export_type = reader.read_u8()?;
                    if export_type == JS_EXPORT_TYPE_LOCAL {
                        // The local entity of the export.
                        let var_idx = reader.read_leb128()?;
                        let export_name_idx = reader.read_atom()?;
                        exports.push(ModuleExportEntry::Local {
                            var_idx,
                            export_name_idx,
                        });
                    } else {
                        let module_idx = reader.read_leb128()?;
                        let local_name_idx = reader.read_atom()?;
                        let export_name_idx = reader.read_atom()?;
                        exports.push(ModuleExportEntry::Indirect {
                            module_idx,
                            local_name_idx,
                            export_name_idx,
                        });
                    }
                    Ok(())
                })
            })
            // Star exports count.
            .and_then(|_| reader.read_leb128())
            // Each * export
            .and_then(|count| {
                (0..count).try_for_each(|_| {
                    star_exports.push(reader.read_leb128()?);
                    Ok(())
                })
            })
            // Imports count.
            .and_then(|_| reader.read_leb128())
            // Each import.
            .and_then(|imports_count| {
                (0..imports_count).try_for_each(|_| {
                    // Variable entity.
                    let var_idx = reader.read_leb128()?;
                    // Import name entity;
                    let name_idx = reader.read_atom()?;
                    // Required module entity.
                    let req_module_idx = reader.read_leb128()?;
                    imports.push(ModuleImportEntry {
                        var_idx,
                        name_idx,
                        req_module_idx,
                    });
                    Ok(())
                })
            })
            // has_tla
            .and_then(|_| reader.read_u8())?;
        Ok(Payload::ModuleHeader(ModuleSectionHeader::new(
            name_entity,
            req_modules,
            exports,
            star_exports,
            imports,
            has_tla,
        )))
    }
}
