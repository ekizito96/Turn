//! Runtime values for the Turn VM.

use serde::{Deserialize, Serialize};
use indexmap::IndexMap;
use std::sync::Arc;
use crate::bytecode::Instr;

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    List(Vec<Value>),
    Map(IndexMap<String, Value>),
    Closure {
        code: Arc<Vec<Instr>>,
        ip: usize,
        env: HashMap<String, Value>,
    },
}

impl Value {
    pub fn is_falsy(&self) -> bool {
        match self {
            Value::Bool(b) => !*b,
            Value::Null => true,
            Value::Str(s) => s.is_empty(),
            Value::Num(n) => *n == 0.0 || *n == -0.0,
            Value::List(l) => l.is_empty(),
            Value::Map(m) => m.is_empty(),
            Value::Closure { .. } => false,
        }
    }

    pub fn is_truthy(&self) -> bool {
        !self.is_falsy()
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Num(n) => write!(f, "{}", n),
            Value::Str(s) => write!(f, "{}", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Null => write!(f, "null"),
            Value::Closure { ip, .. } => write!(f, "<closure at {}>", ip),
            Value::List(l) => {
                write!(f, "[")?;
                for (i, v) in l.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Map(m) => {
                write!(f, "{{")?;
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, "}}")
            }
        }
    }
}
