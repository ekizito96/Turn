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
    
    // Structs
    DefineStruct(String, indexmap::IndexMap<String, Type>),
    
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
    MakeTurn(u32, bool, Vec<(String, Option<Type>, bool)>),

    // Concurrency
    Spawn,
    SpawnRemote,
    Send,
    Receive,
    Link, // NEW: Bidirectional lifecycle binding
    Monitor, // NEW: Unidirectional death notification
    Confidence, // NEW
    Infer(Type, u32), // NEW
    Suspend, // NEW
    
    // Control flow
    Jump(u32),
    JumpIfFalse(u32),
    JumpIfTrue(u32),

    MakeOk, // NEW
    MakeErr, // NEW
    MatchResult(u32), // NEW: jumps to offset if Err, continues if Ok

    // Turn
    EnterTurn(u32), // address to jump to when turn returns
    Return,
}
