//! Agent state and runtime per spec/03-runtime-model.md.

use crate::value::Value;
use std::collections::HashMap;

const MAX_CONTEXT_SIZE: usize = 100;

#[derive(Debug, Clone, Default)]
pub struct Runtime {
    pub env: HashMap<String, Value>,
    pub context: Vec<Value>,
    pub memory: HashMap<String, Value>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            env: HashMap::new(),
            context: Vec::new(),
            memory: HashMap::new(),
        }
    }

    pub fn append_context(&mut self, value: Value) -> Result<(), RuntimeError> {
        if self.context.len() >= MAX_CONTEXT_SIZE {
            // Evict oldest (per spec: either evict or fail)
            self.context.remove(0);
        }
        self.context.push(value);
        Ok(())
    }

    pub fn remember(&mut self, key: Value, val: Value) -> Result<(), RuntimeError> {
        let key_str = value_to_key(&key)?;
        self.memory.insert(key_str, val);
        Ok(())
    }

    pub fn recall(&self, key: &Value) -> Value {
        let key_str = match value_to_key(key) {
            Ok(s) => s,
            Err(_) => return Value::Null,
        };
        self.memory.get(&key_str).cloned().unwrap_or(Value::Null)
    }

    pub fn push_env(&mut self, name: String, value: Value) {
        self.env.insert(name, value);
    }

    pub fn get_env(&self, name: &str) -> Option<Value> {
        self.env.get(name).cloned()
    }

    pub fn pop_env(&mut self, name: &str) -> Option<Value> {
        self.env.remove(name)
    }
}

fn value_to_key(v: &Value) -> Result<String, RuntimeError> {
    match v {
        Value::Str(s) => Ok(s.clone()),
        Value::Num(n) => Ok(n.to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Null => Err(RuntimeError::InvalidMemoryKey),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("invalid memory key (must be string, number, or bool)")]
    InvalidMemoryKey,
}
