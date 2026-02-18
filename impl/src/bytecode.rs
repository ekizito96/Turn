//! Bytecode instruction definitions for the Turn VM.

use serde::{Deserialize, Serialize};
use crate::ast::Type;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Instr {
    // Type checking
    CheckType(Type),

    // Stack ops
    PushNum(f64),
    PushStr(String),
    PushTrue,
    PushFalse,
    PushNull,
    MakeList(usize),
    MakeMap(usize),
    MakeStruct(String, usize),
    MakeVec(usize),
    
    // Variables
    Load(String),
    Store(String),

    // Binary ops
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    Not,
    Similarity, // ~>
    
    // Control
    Pop, // discard top of stack

    // Agent primitives
    ContextAppend,
    Remember,
    Recall,
    CallTool,
    CallMethod(String), // NEW
    LoadModule,
    Index,
    MakeTurn(u32, Vec<String>),

    // Concurrency
    Spawn,
    Send,
    Receive,
    Confidence, // NEW
    Infer(Type), // NEW
    
    // Control flow
    Jump(u32),
    JumpIfFalse(u32),
    JumpIfTrue(u32),

    PushHandler(u32), // offset to catch block
    PopHandler,
    Throw,

    // Turn
    EnterTurn(u32), // address to jump to when turn returns
    Return,
}
