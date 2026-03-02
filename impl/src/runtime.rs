//! Agent state and runtime per spec/03-runtime-model.md.

use crate::value::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::ast::Type;
use indexmap::IndexMap;

const MAX_CONTEXT_SIZE: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StructuredContext {
    pub p0_system: Vec<Value>,
    pub p1_working: std::collections::VecDeque<Value>,
    pub p2_episodic: Vec<Value>,
}

impl StructuredContext {
    pub fn push_system(&mut self, value: Value) {
        self.p0_system.push(value);
    }

    pub fn push_working(&mut self, value: Value, max_size: usize) {
        if self.p1_working.len() >= max_size {
            // Evict oldest working memory to episodic memory
            if let Some(old) = self.p1_working.pop_front() {
                self.p2_episodic.push(old);
            }

            // If episodic memory grows too large, we could compress it here.
            // For now, we will simply limit its size to prevent OOM
            if self.p2_episodic.len() > max_size * 2 {
                self.p2_episodic.remove(0); // Evict oldest episodic
            }
        }
        self.p1_working.push_back(value);
    }

    // Renders the context flat, respecting P0 > P1 > P2 priority
    pub fn to_flat_vec(&self) -> Vec<Value> {
        let mut flat = Vec::new();
        // P0 System Prompt at top
        flat.extend(self.p0_system.iter().cloned());
        // P2 Episodic History in middle
        flat.extend(self.p2_episodic.iter().cloned());
        // P1 Working Memory at bottom (most recent)
        flat.extend(self.p1_working.iter().cloned());
        flat
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Runtime {
    pub env: HashMap<String, Value>,
    pub context: StructuredContext,
    pub memory: HashMap<String, Value>,
    pub structs: HashMap<String, IndexMap<String, Type>>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            env: HashMap::new(),
            context: StructuredContext::default(),
            memory: HashMap::new(),
            structs: HashMap::new(),
        }
    }

    pub fn register_struct(&mut self, name: String, fields: IndexMap<String, Type>) {
        self.structs.insert(name, fields);
    }

    pub fn system_context(&mut self, value: Value) -> Result<(), RuntimeError> {
        self.context.push_system(value);
        Ok(())
    }

    pub fn append_context(&mut self, value: Value) -> Result<(), RuntimeError> {
        self.context.push_working(value, MAX_CONTEXT_SIZE);
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
        Value::List(_)
        | Value::Map(_)
        | Value::Struct(_, _)
        | Value::Closure { .. }
        | Value::Pid(_)
        | Value::Vec(_)
        | Value::Uncertain(..) => Err(RuntimeError::InvalidMemoryKey),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("invalid memory key (must be string, number, or bool)")]
    InvalidMemoryKey,
}
