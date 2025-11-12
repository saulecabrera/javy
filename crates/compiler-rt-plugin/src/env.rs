// Until everything here is used.
#![allow(dead_code)]

//! Runtime environment.

use javy_plugin_api::javy::{quickjs::Value, Runtime};
use std::{cell::RefCell, rc::Rc};

/// Shared value reference.
struct VarRef<'ctx> {
    /// The value slot.
    slot: Rc<RefCell<Value<'ctx>>>,
}

impl<'ctx> VarRef<'ctx> {
    fn new(val: Value<'ctx>) -> Self {
        Self {
            slot: Rc::new(RefCell::new(val)),
        }
    }

    fn set(&self, value: Value<'ctx>) {
        *self.slot.borrow_mut() = value;
    }

    fn get(&self) -> Value<'ctx> {
        self.slot.borrow().clone()
    }
}

/// Funtion frame.
#[derive(Default)]
pub(crate) struct Frame<'ctx> {
    /// References to function locals.
    locals: Vec<VarRef<'ctx>>,
    /// References to function arguments.
    args: Vec<VarRef<'ctx>>,
}

impl<'ctx> Frame<'ctx> {
    fn add_local(&mut self, val: Value<'ctx>) {
        self.locals.push(VarRef::new(val));
    }

    fn add_arg(&mut self, val: Value<'ctx>) {
        self.args.push(VarRef::new(val));
    }
}

pub(crate) struct FuncEnvHandle(usize);
impl FuncEnvHandle {
    pub fn from_usize(n: usize) -> Self {
        Self(n)
    }

    pub fn as_usize(&self) -> usize {
        self.0
    }
}

#[derive(Default)]
struct FuncEnv<'ctx> {
    /// Value references.
    var_refs: Vec<VarRef<'ctx>>,
}

pub(crate) struct CompilerRuntime<'ctx> {
    /// JavaScript runtime.
    pub(crate) inner: Runtime,
    /// Function frames.
    frames: Vec<Frame<'ctx>>,
    /// A list of metadata, per-function.
    func_envs: Vec<FuncEnv<'ctx>>,
}

impl<'ctx> CompilerRuntime<'ctx> {
    /// Create a new environment.
    pub fn new() -> Self {
        Self {
            frames: vec![],
            func_envs: vec![],
            inner: Runtime::default(),
        }
    }

    /// Push a new frame.
    pub fn push_frame(&mut self) {
        self.frames.push(Default::default())
    }

    /// Create a new function environment.
    pub fn new_env(&mut self) -> FuncEnvHandle {
        let index = self.func_envs.len();
        let result = FuncEnvHandle::from_usize(index);
        self.func_envs.push(Default::default());
        result
    }

    fn current_frame(&self) -> &Frame<'ctx> {
        self.frames
            .last()
            .as_ref()
            .expect("Current frame to be available")
    }
}

#[cfg(test)]
mod tests {
    use javy_plugin_api::javy::{
        quickjs::{qjs, Value},
        Runtime,
    };
}
