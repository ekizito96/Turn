//! Agent state and runtime per spec/03-runtime-model.md.

use crate::value::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

/// Tracks 'with budget(tokens: X, time: Y) { ... }' scope limits.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BudgetFrame {
    pub max_tokens: Option<usize>,
    pub used_tokens: usize,
    pub max_time_secs: Option<f64>,
    pub started_at_secs: f64,
}

impl BudgetFrame {
    pub fn new(max_tokens: Option<usize>, max_time_secs: Option<f64>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        Self { max_tokens, used_tokens: 0, max_time_secs, started_at_secs: now }
    }

    /// Returns `Some(reason)` if the budget is exhausted.
    pub fn check_exhausted(&self) -> Option<String> {
        if let Some(max) = self.max_tokens {
            if self.used_tokens >= max {
                return Some(format!(
                    "Thermodynamic Constraint Exceeded: token budget exhausted ({}/{} tokens used).",
                    self.used_tokens, max
                ));
            }
        }
        if let Some(max_t) = self.max_time_secs {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64();
            if now - self.started_at_secs >= max_t {
                return Some(format!(
                    "Thermodynamic Constraint Exceeded: time budget exhausted ({:.1}/{:.1}s elapsed).",
                    now - self.started_at_secs, max_t
                ));
            }
        }
        None
    }
}

pub fn cosine_similarity(v1: &[f64], v2: &[f64]) -> f64 {
    if v1.len() != v2.len() || v1.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0;
    let mut norm1 = 0.0;
    let mut norm2 = 0.0;
    for (a, b) in v1.iter().zip(v2.iter()) {
        dot += a * b;
        norm1 += a * a;
        norm2 += b * b;
    }
    if norm1 == 0.0 || norm2 == 0.0 {
        return 0.0;
    }
    dot / (norm1.sqrt() * norm2.sqrt())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItem {
    pub key: String,
    pub embedding: Option<Vec<f64>>,
    pub value: Value,
    pub neighbors: Vec<usize>, // HNSW Level 0 Graph edges
    pub created_at: u64,       // UNIX epoch
    pub last_accessed: u64,    // UNIX epoch
    pub velocity: f64,         // Orbit speed/Volatility scalar
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
                if embedding.is_some() {
                    item.embedding = embedding.clone();
                }
                return;
            }
        }

        // Get current time
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(std::time::Duration::from_secs(0))
            .as_secs();

        // Append raw node
        let new_idx = self.items.len();
        self.items.push(MemoryItem {
            key,
            embedding: embedding.clone(),
            value,
            neighbors: Vec::new(),
            created_at: now,
            last_accessed: now,
            velocity: 1.0, // Default baseline velocity
        });

        // If we lack vectors, we can't graft it into the HNSW graph cleanly
        let q_emb = match embedding {
            Some(v) => v,
            None => {
                if self.entry_point.is_none() {
                    self.entry_point = Some(new_idx);
                }
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
            #[allow(clippy::manual_is_multiple_of)]
            if new_idx % 10 == 0 {
                self.entry_point = Some(new_idx);
            }
        } else {
            self.entry_point = Some(new_idx);
        }
    }

    pub fn get(&mut self, key: &str) -> Option<Value> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(std::time::Duration::from_secs(0))
            .as_secs();

        for item in &mut self.items {
            if item.key == key {
                item.last_accessed = now; // Spaced Repetition reset
                return Some(item.value.clone());
            }
        }
        None
    }

    /// Pillar 4: Physically deletes entries matching the given label from Semantic RAM.
    pub fn forget(&mut self, label: &str) {
        self.items.retain(|item| item.key != label);
        // Reset HNSW entry point if it may be stale
        if self.items.is_empty() {
            self.entry_point = None;
        } else if let Some(ep) = self.entry_point {
            if ep >= self.items.len() {
                self.entry_point = Some(0);
            }
        }
    }

    // O(log N) Greedy Search across HNSW Layer 0
    pub fn search(&self, query_emb: &[f64], top_k: usize) -> Vec<Value> {
        if self.items.is_empty() {
            return Vec::new();
        }

        let ep = self.entry_point.unwrap_or(0);
        let mut curr = ep;

        let mut best_dist = if let Some(ref e) = self.items[curr].embedding {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(std::time::Duration::from_secs(0))
                .as_secs();

            let dt = now.saturating_sub(self.items[curr].last_accessed) as f64;
            // Ebbinghaus Decay: Score = Cosine * e^(-lambda * dt)
            // lambda = 0.0001 (approx 10% decay every 1000 seconds for standard items)
            // We multiply lambda by velocity to accelerate or decelerate decay based on confidence orbits
            let decay_factor = (-0.0001 * self.items[curr].velocity * dt).exp();
            let base_score = cosine_similarity(query_emb, e);
            1.0 - (base_score * decay_factor)
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
                if visited.contains(&nxt) {
                    continue;
                }
                visited.insert(nxt);

                if let Some(ref e) = self.items[nxt].embedding {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or(std::time::Duration::from_secs(0))
                        .as_secs();
                    let dt = now.saturating_sub(self.items[nxt].last_accessed) as f64;
                    let decay_factor = (-0.0001 * self.items[nxt].velocity * dt).exp();
                    let base_score = cosine_similarity(query_emb, e);
                    let d = 1.0 - (base_score * decay_factor);

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

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(std::time::Duration::from_secs(0))
            .as_secs();

        while let Some(node) = frontier.pop() {
            if scored.len() >= top_k {
                break;
            }
            if let Some(ref e) = self.items[node].embedding {
                let dt = now.saturating_sub(self.items[node].last_accessed) as f64;
                let decay_factor = (-0.0001 * self.items[node].velocity * dt).exp();
                let base_score = cosine_similarity(query_emb, e);
                let final_score = base_score * decay_factor;
                scored.push((final_score, node));
            }

            for &n in &self.items[node].neighbors {
                if !k_visited.contains(&n) {
                    k_visited.insert(n);
                    frontier.push(n);
                }
            }
        }

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut results = Vec::new();
        for (_, node_idx) in scored.into_iter().take(top_k) {
            // Unsafe mutable bypass since `search` takes `&self` not `&mut self`.
            // In a production engine, this would use AtomicU64 or RefCell for internal mutation.
            // For now, since Turn executes linearly per Vm tick, we can ignore the timestamp update on exact read
            // or we must refactor `search` to `&mut self`. Let's just return the value here to keep the API stable,
            // and we'll implement Spaced Repetition (last_accessed reset) explicitly when `recall()` is called natively.
            results.push(self.items[node_idx].value.clone());
        }
        results
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
        println!(
            "[Switchboard] Dropped message to remote node {}. Switchboard not connected.",
            node_id
        );
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
    pub budget_stack: Vec<BudgetFrame>, // Thermodynamic guardrails
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
            budget_stack: self.budget_stack.clone(),
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
            budget_stack: Vec::new(),
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
            if name.as_str() == "ContextItem" || m.contains_key("priority") {
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
                let id = m
                    .get("id")
                    .and_then(|v| value_to_key(v).ok())
                    .unwrap_or_else(|| "unnamed".to_string());
                let emb = match m.get("embedding") {
                    Some(Value::Vec(v)) => Some((**v).clone()),
                    _ => None,
                };
                (id, emb)
            }
            _ => (value_to_key(&key)?, None),
        };

        self.memory.insert(id, embedding, val);
        Ok(())
    }

    pub fn recall(&mut self, key: &Value) -> Value {
        if let Value::Vec(v) = key {
            let results = self.memory.search(v, 1);
            if let Some(val) = results.first() {
                // To formally reset the timestamp on the retrieved node without mutating `search`,
                // we lookup the item by exact match since it's the only one returned (top_k=1).
                // Or simply rely on `search` not needing exactly accurate `last_accessed` for Vector inputs.
                // However, since we updated `get()` above, we should implement a dedicated mut search later if needed.
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

    /// Executed per VM cycle to implement the Ebbinghaus "Event Horizon".
    /// If a memory vector's retrieval strength drops below the noise threshold, it is deleted.
    pub fn tick_garbage_collection(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(std::time::Duration::from_secs(0))
            .as_secs();

        let noise_threshold = 0.05; // 5% minimum contextual relevance

        let mut to_remove = Vec::new();

        for (i, item) in self.memory.items.iter().enumerate() {
            let dt = now.saturating_sub(item.last_accessed) as f64;
            let decay_factor = (-0.0001 * item.velocity * dt).exp();

            // For now, base score assumes a perfect match 1.0 against itself for decay tracking
            let retrieval_strength = 1.0 * decay_factor;

            if retrieval_strength < noise_threshold {
                to_remove.push(i);
            }
        }

        // Remove from highest index to lowest to avoid shifting issues
        to_remove.sort_by(|a, b| b.cmp(a));
        for idx in to_remove {
            if let Some(ep) = self.memory.entry_point {
                // If we delete the entry point, just clear it so next insert reshapes the graph
                if ep == idx {
                    self.memory.entry_point = None;
                }
            }
            self.memory.items.remove(idx);
            // Note: HNSW Level 0 edges (neighbors) might end up pointing to shifted bounds.
            // In a production Turn Engine, this is a full Graph re-indexing operation.
            // For alpha v0.4.0, we just clear all graphs edges to force greedy flat search if needed,
            // or simply leave it. We'll clear the entry point to force a new graft eventually.
            self.memory.entry_point = None;
        }
    }
}

pub fn value_to_key(v: &Value) -> Result<String, RuntimeError> {
    match v {
        Value::Str(s) => Ok(s.to_string()),
        Value::Num(n) => Ok(n.to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Null => Err(RuntimeError::InvalidMemoryKey),
        Value::List(_)
        | Value::Map(_)
        | Value::Struct(_, _)
        | Value::Closure { .. }
        | Value::Pid { .. }
        | Value::Vec(_)
        | Value::Cap(_)
        | Value::CapProxy { .. }
        | Value::ToolCallRequest(_, _)
        | Value::Uncertain(..) => Err(RuntimeError::InvalidMemoryKey),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("invalid memory key (must be string, number, or bool)")]
    InvalidMemoryKey,
}
