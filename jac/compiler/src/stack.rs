//! Abstract stack definition.

use waffle::{Type, Value};

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
}
