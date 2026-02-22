//! Agent state and runtime per spec/03-runtime-model.md.

use crate::value::Value;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::ast::Type;
use indexmap::IndexMap;

const DEFAULT_TOKEN_BUDGET: usize = 8000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextItem {
    pub priority: u8,
    pub token_cost: usize,
    pub value: Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PriorityStack {
    pub items: Vec<ContextItem>,
    pub token_budget: usize,
    pub current_tokens: usize,
}

impl PriorityStack {
    pub fn new(budget: usize) -> Self {
        Self {
            items: Vec::new(),
            token_budget: budget,
            current_tokens: 0,
        }
    }

    pub fn push(&mut self, item: ContextItem) {
        // Enforce Entropic Eviction Rules
        // Drop lowest priority first (highest u8)
        while self.current_tokens + item.token_cost > self.token_budget && !self.items.is_empty() {
            let mut worst_prio = 0;
            let mut worst_idx = 0;
            for (i, existing) in self.items.iter().enumerate() {
                if existing.priority >= worst_prio {
                    worst_prio = existing.priority;
                    worst_idx = i;
                }
            }
            if worst_prio == 0 {
                // P0 is inviolable. Break and overflow
                break;
            }
            let evicted = self.items.remove(worst_idx);
            self.current_tokens -= evicted.token_cost;
        }
        
        self.current_tokens += item.token_cost;
        self.items.push(item);
    }

    pub fn to_values(&self) -> Vec<Value> {
        self.items.iter().map(|i| i.value.clone()).collect()
    }
}

fn estimate_tokens(val: &Value) -> usize {
    let s = val.to_string();
    (s.len() / 4) + 1
}

pub fn cosine_similarity(v1: &[f64], v2: &[f64]) -> f64 {
    if v1.len() != v2.len() || v1.is_empty() { return 0.0; }
    let mut dot = 0.0;
    let mut norm1 = 0.0;
    let mut norm2 = 0.0;
    for (a, b) in v1.iter().zip(v2.iter()) {
        dot += a * b;
        norm1 += a * a;
        norm2 += b * b;
    }
    if norm1 == 0.0 || norm2 == 0.0 { return 0.0; }
    dot / (norm1.sqrt() * norm2.sqrt())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItem {
    pub key: String,
    pub embedding: Option<Vec<f64>>,
    pub value: Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SemanticMemory {
    pub items: Vec<MemoryItem>,
}

impl SemanticMemory {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }
    
    pub fn insert(&mut self, key: String, embedding: Option<Vec<f64>>, value: Value) {
        for item in &mut self.items {
            if item.key == key {
                item.value = value;
                item.embedding = embedding;
                return;
            }
        }
        self.items.push(MemoryItem { key, embedding, value });
    }
    
    pub fn get(&self, key: &str) -> Option<Value> {
        for item in &self.items {
            if item.key == key { return Some(item.value.clone()); }
        }
        None
    }
    
    pub fn search(&self, query_emb: &[f64], top_k: usize) -> Vec<Value> {
        let mut scored = Vec::new();
        for item in &self.items {
            if let Some(ref emb) = item.embedding {
                let score = cosine_similarity(query_emb, emb);
                scored.push((score, item.value.clone()));
            }
        }
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(top_k).map(|(_, v)| v).collect()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Runtime {
    pub env: HashMap<String, Value>,
    pub context: PriorityStack,
    pub memory: SemanticMemory,
    pub structs: HashMap<String, IndexMap<String, Type>>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            env: HashMap::new(),
            context: PriorityStack::new(DEFAULT_TOKEN_BUDGET),
            memory: SemanticMemory::new(),
            structs: HashMap::new(),
        }
    }

    pub fn register_struct(&mut self, name: String, fields: IndexMap<String, Type>) {
        self.structs.insert(name, fields);
    }

    pub fn append_context(&mut self, value: Value) -> Result<(), RuntimeError> {
        let mut priority = 2; // Default to P2 (History)
        let mut actual_value = value.clone();
        
        if let Value::Struct(ref name, ref m) = value {
            if name == "ContextItem" || m.contains_key("priority") {
                if let Some(Value::Num(p)) = m.get("priority") {
                    priority = *p as u8;
                }
                if let Some(v) = m.get("content") {
                    actual_value = v.clone();
                } else if let Some(v) = m.get("value") {
                    actual_value = v.clone();
                }
            }
        } else if let Value::Map(ref m) = value {
            if let Some(Value::Num(p)) = m.get("priority") {
                priority = *p as u8;
            }
            if let Some(v) = m.get("content") {
                actual_value = v.clone();
            } else if let Some(v) = m.get("value") {
                actual_value = v.clone();
            }
        }
        
        let token_cost = estimate_tokens(&actual_value);
        self.context.push(ContextItem {
            priority,
            token_cost,
            value: actual_value,
        });
        
        Ok(())
    }

    pub fn remember(&mut self, key: Value, val: Value) -> Result<(), RuntimeError> {
        let (id, embedding) = match key {
            Value::Map(ref m) | Value::Struct(_, ref m) => {
                let id = m.get("id").and_then(|v| value_to_key(v).ok()).unwrap_or_else(|| "unnamed".to_string());
                let emb = match m.get("embedding") {
                    Some(Value::Vec(v)) => Some(v.clone()),
                    _ => None,
                };
                (id, emb)
            }
            _ => (value_to_key(&key)?, None)
        };
        
        self.memory.insert(id, embedding, val);
        Ok(())
    }

    pub fn recall(&self, key: &Value) -> Value {
        if let Value::Vec(v) = key {
            let results = self.memory.search(v, 1);
            if let Some(val) = results.first() {
                return val.clone();
            }
            return Value::Null;
        }
        
        let key_str = match value_to_key(key) {
            Ok(s) => s,
            Err(_) => return Value::Null,
        };
        self.memory.get(&key_str).unwrap_or(Value::Null)
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
        Value::List(_) | Value::Map(_) | Value::Struct(_, _) | Value::Closure { .. } | Value::Pid(_) | Value::Vec(_) | Value::Uncertain(..) => Err(RuntimeError::InvalidMemoryKey),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("invalid memory key (must be string, number, or bool)")]
    InvalidMemoryKey,
}
