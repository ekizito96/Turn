//! Runtime values for the Turn VM.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Num(f64),
    Str(String),
    Bool(bool),
    Null,
}

impl Value {
    pub fn is_falsy(&self) -> bool {
        match self {
            Value::Bool(b) => !*b,
            Value::Null => true,
            Value::Str(s) => s.is_empty(),
            Value::Num(n) => *n == 0.0 || *n == -0.0,
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
        }
    }
}
