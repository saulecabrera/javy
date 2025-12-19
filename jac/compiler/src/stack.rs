//! Abstract stack definition.

use crate::compiler::FuncEnv;
use waffle::{Operator, Type, Value};

/// Abstract stack value.
pub(crate) struct StackVal {
    /// The inner SSA value.
    pub val: Value,
    /// The associated type.
    pub ty: Option<Type>,
}

impl StackVal {
    /// Create a new stack val.
    pub fn new(val: Value, ty: Option<Type>) -> Self {
        Self { val, ty }
    }

    pub fn i32(val: Value) -> Self {
	Self  { val, ty: Some(Type::I32) }
    }

    pub fn void(val: Value) -> Self {
	Self { val, ty: None }
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

    /// Peeks the top `n` elements in the stack.
    pub fn peekn(&self, n: usize) -> &[StackVal] {
        assert!(n <= self.inner.len());

        let start = self.inner.len().checked_sub(n).unwrap();
        &self.inner[start..]
    }

    /// Drops the last `n` elements from the stack.
    pub fn drop(&mut self, n: usize) {
        self.inner.truncate(n);
    }
}
