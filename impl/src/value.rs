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
    Struct(String, IndexMap<String, Value>),
    Closure {
        code: Arc<Vec<Instr>>,
        ip: usize,
        env: HashMap<String, Value>,
        params: Vec<String>,
    },
    Pid(u64), // Process ID
    Vec(Vec<f64>),
    Uncertain(Box<Value>, f64), // Value, Confidence (0.0 - 1.0)
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
            Value::Struct(_, m) => m.is_empty(),
            Value::Pid(_) => false,
            Value::Vec(v) => v.is_empty(),
            Value::Uncertain(v, _) => v.is_falsy(),
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
            Value::Struct(name, m) => {
                write!(f, "{} {{", name)?;
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, "}}")
            }
            Value::Pid(id) => write!(f, "<pid {}>", id),
            Value::Vec(v) => {
                write!(f, "vec[")?;
                for (i, val) in v.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", val)?;
                }
                write!(f, "]")
            }
            Value::Uncertain(v, p) => write!(f, "{} ({}%)", v, p * 100.0),
        }
    }
}
