use waffle::Block;

/// The current phase of a conditional control frame.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum CondState {
    /// Currently emitting into the alternative (fallthrough/if-true) block.
    Alt,
    /// Currently emitting into the consequent (if-false) block.
    Consequent,
}

/// A conditional control frame.
#[derive(Copy, Clone)]
pub struct Cond {
    /// The current phase.
    pub state: CondState,
    /// The alternative (fallthrough) block.
    pub alt: Block,
    /// The consequent (branch target) block.
    pub consequent: Block,
    /// Optional join block, set when a GoTo is encountered.
    pub join: Option<Block>,
    /// Optional bytecode offset signalling the end of the consequent.
    pub end: Option<usize>,
}

impl Cond {
    pub fn new(alt: Block, consequent: Block) -> Self {
        Self {
            state: CondState::Alt,
            alt,
            consequent,
            join: None,
            end: None,
        }
    }
}

pub enum ControlFrame {
    Cond(Cond),
}

pub struct ControlStack {
    frames: Vec<ControlFrame>,
}

impl ControlStack {
    pub fn new() -> Self {
        Self { frames: vec![] }
    }

    pub fn push(&mut self, frame: ControlFrame) {
        self.frames.push(frame);
    }

    pub fn peek(&self) -> Option<&ControlFrame> {
        self.frames.last()
    }

    pub fn peek_mut(&mut self) -> Option<&mut ControlFrame> {
        self.frames.last_mut()
    }

    pub fn pop(&mut self) -> ControlFrame {
        self.frames.pop().unwrap()
    }
}
