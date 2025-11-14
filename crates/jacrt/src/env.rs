// Until everything here is used.
#![allow(dead_code)]

//! Runtime environment.

use javy_plugin_api::javy::{
    quickjs::{qjs, Ctx, Value},
    Runtime,
};
use std::collections::HashMap;
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

    fn get(&self) -> Self {
        VarRef {
            slot: Rc::clone(&self.slot),
        }
    }

    pub fn get_value(&self) -> Value<'ctx> {
        self.slot.borrow().clone()
    }
}

/// Funtion frame.
#[derive(Default)]
pub struct Frame<'ctx> {
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

#[derive(Hash, Eq, PartialEq, Clone, Copy)]
pub struct FuncEnvHandle(usize);
impl FuncEnvHandle {
    pub fn from_usize(n: usize) -> Self {
        Self(n)
    }

    pub fn as_usize(&self) -> usize {
        self.0
    }
}

#[derive(Default)]
pub struct FuncEnv<'ctx> {
    /// Value references.
    var_refs: Vec<VarRef<'ctx>>,
}

pub struct CompilerRuntime<'ctx> {
    /// Function frames.
    frames: Vec<Frame<'ctx>>,
    /// A list of metadata, per-function.
    func_envs: HashMap<FuncEnvHandle, FuncEnv<'ctx>>,
    /// The current function environment.
    current_env: FuncEnvHandle,
}

impl<'ctx> CompilerRuntime<'ctx> {
    /// Setup the initial state of the compiler runtime.
    pub fn init(var_ref_slots: usize) -> Runtime {
        let runtime = Runtime::default();
        runtime.context().with(|cx| {
            let mut var_refs = vec![];
            for _ in 0..var_ref_slots {
                let var_ref = VarRef::new(Value::new_undefined(cx.clone()));
                var_refs.push(var_ref);
            }
            let env = FuncEnv { var_refs };
            let mut func_envs = HashMap::new();
            let current_env = FuncEnvHandle::from_usize(0);
            func_envs.insert(current_env, env);
            let inner = Box::new(CompilerRuntime {
                current_env,
                frames: vec![],
                func_envs,
            });

            let opaque = Box::into_raw(inner);
            // TODO: ensure that this memory gets correctly dropped.
            unsafe { qjs::JS_SetContextOpaque(cx.as_raw().as_ptr(), opaque as _) };
        });
        runtime
    }

    /// Get a mutable reference to the `CompilerRuntime`
    /// stored in the given context.
    pub fn mut_from_context(cx: Ctx<'ctx>) -> &'ctx mut Self {
        unsafe {
            let ptr = qjs::JS_GetContextOpaque(cx.as_raw().as_ptr()) as *mut Self;
            &mut *ptr
        }
    }

    fn get_current_func_env(&self) -> &FuncEnv<'ctx> {
        &self.func_envs[&self.current_env]
    }

    /// Ensures that the variable reference at index is correctly
    /// handled.
    /// Closures are created in the context of the parent function,
    /// from which they are capturing the value references, thus,
    /// the index, in the non-local case is an index to a VarRef
    /// in the current function enviorement.
    pub fn resolve_non_local_var_ref(&mut self, index: usize, target_handle: FuncEnvHandle) {
        let current = self.get_current_func_env();
        let var_ref = current.var_refs[index].get();
        let target = &mut self.func_envs.get_mut(&target_handle).unwrap();
        target.var_refs.push(var_ref)
    }

    /// Set the given value at the given index.
    pub fn put_var_ref(&mut self, index: usize, val: Value<'ctx>) {
        let current = self.get_current_func_env();
        let vref = &current.var_refs[index];
        vref.set(val)
    }

    /// Get the `VarRef` at index.
    pub fn get_var_ref_value(&self, index: usize) -> Value<'ctx> {
        let current = self.get_current_func_env();
        current.var_refs[index].get().get_value()
    }

    /// Push a new frame.
    pub fn push_frame(&mut self) {
        self.frames.push(Default::default())
    }

    /// Create a new function environment.
    pub fn push_default_env(&mut self, handle: FuncEnvHandle) {
        self.func_envs.insert(handle, Default::default());
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
    use super::{CompilerRuntime, FuncEnvHandle};
    use javy_plugin_api::javy::quickjs::Value;

    #[test]
    fn initializes_the_runtime_with_the_given_var_refs() {
        let runtime = CompilerRuntime::init(10);
        runtime.context().with(|cx| {
            let compiler_runtime = CompilerRuntime::mut_from_context(cx);

            assert_eq!(compiler_runtime.func_envs.len(), 1);
            assert_eq!(
                compiler_runtime.func_envs[&FuncEnvHandle::from_usize(0)]
                    .var_refs
                    .len(),
                10
            );

            for vr in &compiler_runtime.func_envs[&FuncEnvHandle::from_usize(0)].var_refs {
                assert!(vr.get_value().is_undefined());
            }
        });
    }

    #[test]
    fn changes_to_var_refs_are_observable() {
        let runtime = CompilerRuntime::init(2);

        runtime.context().with(|cx| {
            let compiler_runtime = CompilerRuntime::mut_from_context(cx.clone());
            // Create a new env, e.g., what would need to happen when
            // creating a closure.
            let main_handle = FuncEnvHandle::from_usize(0);
            let handle = FuncEnvHandle::from_usize(3);
            compiler_runtime.push_default_env(handle);
            // Resolve closure value references.
            compiler_runtime.resolve_non_local_var_ref(0, handle);
            compiler_runtime.put_var_ref(0, Value::new_int(cx.clone(), 42));

            let main = &compiler_runtime.func_envs[&main_handle];
            let closure = &compiler_runtime.func_envs[&handle];

            assert_eq!(main.var_refs.len(), 2);
            assert_eq!(closure.var_refs.len(), 1);

            assert!(main.var_refs[0].get_value().is_int());
            assert!(main.var_refs[1].get_value().is_undefined());

            assert!(closure.var_refs[0].get_value().is_int());

            assert!(closure.var_refs[0].get_value() == main.var_refs[0].get_value());
        })
    }
}
