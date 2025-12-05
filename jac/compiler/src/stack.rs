//! Abstract stack definition.

use crate::compiler::FuncEnv;
use waffle::{Operator, Type, Value};

/// Abstract stack value.
pub(crate) struct StackVal {
    /// The inner SSA value.
    pub val: Value,
    /// The associated type.
    pub ty: Type,
}

impl StackVal {
    /// Create a new stack val.
    pub fn new(val: Value, ty: Type) -> Self {
        Self { val, ty }
    }
}

#[derive(Default)]
pub(crate) struct Stack {
    inner: Vec<StackVal>,
}

impl Stack {
    /// Pop one value from the stack.
    pub fn pop1(&mut self) -> StackVal {
        self.inner.pop().expect("at least one value in the stack")
    }

    /// Push one value to the stack.
    pub fn push(&mut self, val: StackVal) {
        self.inner.push(val)
    }

    /// Get the operator value arguments.
    // TODO: We can potentially do better here and avoid returning a
    // `Vec` all the time.
    pub fn get_op_args(&mut self, op: Operator, env: &FuncEnv) -> (Vec<Value>, Type) {
        match op {
            Operator::I32Const { .. } => (vec![], Type::I32),
            Operator::Call { function_index } => {
                let decl = &env.result.funcs[function_index];
                let sig = &env.result.signatures[decl.sig()];
                let param_count = sig.params.len();
                let mut args = Vec::with_capacity(param_count);
                for (param, stack_val) in sig
                    .params
                    .iter()
                    .zip(self.inner.iter().rev().take(param_count))
                {
                    assert!(stack_val.ty == *param);
                    args.push(stack_val.val);
                }
                self.inner.truncate(param_count);
                (args, sig.returns[0])
            }
            _ => todo!(),
        }
    }
}
