//! Bytecode instruction definitions for the Turn VM.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Instr {
    // Stack ops
    PushNum(f64),
    PushStr(String),
    PushTrue,
    PushFalse,
    PushNull,
    MakeList(usize),
    MakeMap(usize),

    // Variables
    Load(String),
    Store(String),

    // Binary ops
    Add,
    Eq,
    Ne,
    And,
    Or,

    // Control
    Pop, // discard top of stack

    // Agent primitives
    ContextAppend,
    Remember,
    Recall,
    CallTool,
    Index,

    // Control flow
    Jump(u32),
    JumpIfFalse(u32),
    JumpIfTrue(u32),

    // Turn
    EnterTurn(u32), // address to jump to when turn returns
    Return,
}
