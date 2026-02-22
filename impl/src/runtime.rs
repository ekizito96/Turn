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
    pub neighbors: Vec<usize>, // HNSW Level 0 Graph edges
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SemanticMemory {
    pub items: Vec<MemoryItem>,
    pub entry_point: Option<usize>, // HNSW entry node
}

impl SemanticMemory {
    pub fn new() -> Self {
        Self { 
            items: Vec::new(),
            entry_point: None,
        }
    }
    
    pub fn insert(&mut self, key: String, embedding: Option<Vec<f64>>, value: Value) {
        // Prevent dupes
        for item in &mut self.items {
            if item.key == key {
                item.value = value.clone();
                if embedding.is_some() { item.embedding = embedding.clone(); }
                return;
            }
        }
        
        // Append raw node
        let new_idx = self.items.len();
        self.items.push(MemoryItem { 
            key, 
            embedding: embedding.clone(), 
            value,
            neighbors: Vec::new(),
        });
        
        // If we lack vectors, we can't graft it into the HNSW graph cleanly
        let q_emb = match embedding {
            Some(v) => v,
            None => {
                if self.entry_point.is_none() { self.entry_point = Some(new_idx); }
                return;
            }
        };
        
        if let Some(ep) = self.entry_point {
            // HNSW greedy-search to find closest existing node to bind to
            let mut curr = ep;
            // Simple heuristic mapping for this Turn alpha: max 5 neighbors
            let mut best_dist = if let Some(ref e) = self.items[curr].embedding {
                1.0 - cosine_similarity(&q_emb, e)
            } else {
                f64::MAX
            };
            
            let mut found_closer = true;
            while found_closer {
                found_closer = false;
                let neighbors = self.items[curr].neighbors.clone();
                for &nxt in &neighbors {
                    if let Some(ref e) = self.items[nxt].embedding {
                        let d = 1.0 - cosine_similarity(&q_emb, e);
                        if d < best_dist {
                            best_dist = d;
                            curr = nxt;
                            found_closer = true;
                        }
                    }
                }
            }
            
            // Connect to local minimum
            self.items[curr].neighbors.push(new_idx);
            self.items[new_idx].neighbors.push(curr);
            
            // Reassign entry point if this is structurally central (mock optimization)
            if new_idx % 10 == 0 {
                self.entry_point = Some(new_idx);
            }
        } else {
            self.entry_point = Some(new_idx);
        }
    }
    
    pub fn get(&self, key: &str) -> Option<Value> {
        for item in &self.items {
            if item.key == key { return Some(item.value.clone()); }
        }
        None
    }
    
    // O(log N) Greedy Search across HNSW Layer 0
    pub fn search(&self, query_emb: &[f64], top_k: usize) -> Vec<Value> {
        if self.items.is_empty() { return Vec::new(); }
        
        let ep = self.entry_point.unwrap_or(0);
        let mut curr = ep;
        
        let mut best_dist = if let Some(ref e) = self.items[curr].embedding {
            1.0 - cosine_similarity(query_emb, e)
        } else {
            f64::MAX
        };
        
        let mut visited = std::collections::HashSet::new();
        visited.insert(curr);
        
        let mut found_closer = true;
        while found_closer {
            found_closer = false;
            let neighbors = self.items[curr].neighbors.clone();
            for &nxt in &neighbors {
                if visited.contains(&nxt) { continue; }
                visited.insert(nxt);
                
                if let Some(ref e) = self.items[nxt].embedding {
                    let d = 1.0 - cosine_similarity(query_emb, e);
                    if d < best_dist {
                        best_dist = d;
                        curr = nxt;
                        found_closer = true;
                    }
                }
            }
        }
        
        // Fetch Top K around local minima
        let mut scored = Vec::new();
        let mut frontier = vec![curr];
        let mut k_visited = std::collections::HashSet::new();
        k_visited.insert(curr);
        
        while let Some(node) = frontier.pop() {
            if scored.len() >= top_k { break; }
            if let Some(ref e) = self.items[node].embedding {
                 let score = cosine_similarity(query_emb, e);
                 scored.push((score, self.items[node].value.clone()));
            }
            
            for &n in &self.items[node].neighbors {
                if !k_visited.contains(&n) {
                    k_visited.insert(n);
                    frontier.push(n);
                }
            }
        }
        
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(top_k).map(|(_, v)| v).collect()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilityRegistry {
    #[serde(skip)]
    pub caps: HashMap<usize, String>,
    pub next_id: usize,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self {
            caps: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn mint(&mut self, secret: String) -> usize {
        let id = self.next_id;
        self.caps.insert(id, secret);
        self.next_id += 1;
        id
    }

    pub fn get(&self, id: usize) -> Option<&String> {
        self.caps.get(&id)
    }
}

pub trait NetworkSwitchboard: std::fmt::Debug + Send + Sync {
    fn send_remote(&self, node_id: &str, local_pid: u64, msg: Value) -> Result<(), RuntimeError>;
    fn register_local_node(&mut self, node_id: String);
}

#[derive(Debug, Clone, Default)]
pub struct NoOpSwitchboard {}

impl NetworkSwitchboard for NoOpSwitchboard {
    fn send_remote(&self, node_id: &str, _local_pid: u64, _msg: Value) -> Result<(), RuntimeError> {
        println!("[Switchboard] Dropped message to remote node {}. Switchboard not connected.", node_id);
        Ok(())
    }
    fn register_local_node(&mut self, _node_id: String) {}
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Runtime {
    pub env: HashMap<String, Value>,
    pub context: PriorityStack,
    pub memory: SemanticMemory,
    pub structs: HashMap<String, IndexMap<String, Type>>,
    pub capabilities: CapabilityRegistry,
    #[serde(skip)]
    pub switchboard: Option<std::sync::Arc<dyn NetworkSwitchboard>>,
}

impl Clone for Runtime {
    fn clone(&self) -> Self {
        Self {
            env: self.env.clone(),
            context: self.context.clone(),
            memory: self.memory.clone(),
            structs: self.structs.clone(),
            capabilities: self.capabilities.clone(),
            switchboard: self.switchboard.clone(),
        }
    }
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            env: HashMap::new(),
            context: PriorityStack::new(DEFAULT_TOKEN_BUDGET),
            memory: SemanticMemory::new(),
            structs: HashMap::new(),
            capabilities: CapabilityRegistry::new(),
            switchboard: None,
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

pub fn value_to_key(v: &Value) -> Result<String, RuntimeError> {
    match v {
        Value::Str(s) => Ok(s.clone()),
        Value::Num(n) => Ok(n.to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Null => Err(RuntimeError::InvalidMemoryKey),
        Value::List(_) | Value::Map(_) | Value::Struct(_, _) | Value::Closure { .. } | Value::Pid { .. } | Value::Vec(_) | Value::Cap(_) | Value::CapProxy { .. } | Value::Uncertain(..) => Err(RuntimeError::InvalidMemoryKey),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("invalid memory key (must be string, number, or bool)")]
    InvalidMemoryKey,
}
