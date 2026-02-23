use crate::ast::Type;
use crate::bytecode::Instr;
use crate::runtime::Runtime;
use crate::value::Value;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Frame {
    pub code: Arc<Vec<Instr>>,
    pub ip: usize,
    pub env: HashMap<String, Value>,
    pub handlers: Vec<u32>, // Stack of catch block offsets relative to code start
}

#[derive(Debug)]
pub enum VmEvent {
    Complete {
        pid: u64,
        result: Value,
    },
    Error {
        pid: u64,
        error: String,
    },
    Suspend {
        pid: u64,
        tool_name: String,
        arg: Value,
        resume_tx: tokio::sync::oneshot::Sender<Value>,
        continuation: Box<Option<VmState>>,
    },
}

#[derive(Clone)]
pub struct Registry {
    pub pids: Arc<std::sync::RwLock<HashMap<u64, tokio::sync::mpsc::UnboundedSender<Value>>>>,
    pub next_pid: Arc<std::sync::RwLock<u64>>,
    pub links: Arc<std::sync::RwLock<HashMap<u64, Vec<u64>>>>, // watched -> watchers
    pub monitors: Arc<std::sync::RwLock<HashMap<u64, Vec<u64>>>>, // watched -> watchers
    pub host_tx: tokio::sync::mpsc::UnboundedSender<VmEvent>,
}

impl Registry {
    pub fn new(host_tx: tokio::sync::mpsc::UnboundedSender<VmEvent>) -> Self {
        Self {
            pids: Arc::new(std::sync::RwLock::new(HashMap::new())),
            next_pid: Arc::new(std::sync::RwLock::new(2)), // Root is 1
            links: Arc::new(std::sync::RwLock::new(HashMap::new())),
            monitors: Arc::new(std::sync::RwLock::new(HashMap::new())),
            host_tx,
        }
    }
    pub fn register(&self, pid: u64, tx: tokio::sync::mpsc::UnboundedSender<Value>) {
        self.pids.write().unwrap().insert(pid, tx);
    }
    pub fn unregister(&self, pid: u64) {
        self.pids.write().unwrap().remove(&pid);
    }
    pub fn send(&self, pid: u64, msg: Value) -> bool {
        if let Some(tx) = self.pids.read().unwrap().get(&pid) {
            tx.send(msg).is_ok()
        } else {
            false
        }
    }
    pub fn get_next_pid(&self) -> u64 {
        let mut n = self.next_pid.write().unwrap();
        let pid = *n;
        *n += 1;
        pid
    }
    pub fn add_link(&self, watched: u64, watcher: u64) {
        self.links
            .write()
            .unwrap()
            .entry(watched)
            .or_default()
            .push(watcher);
    }
    pub fn add_monitor(&self, watched: u64, watcher: u64) {
        self.monitors
            .write()
            .unwrap()
            .entry(watched)
            .or_default()
            .push(watcher);
    }
    pub fn get_links(&self, watched: u64) -> Vec<u64> {
        self.links
            .read()
            .unwrap()
            .get(&watched)
            .cloned()
            .unwrap_or_default()
    }
    pub fn get_monitors(&self, watched: u64) -> Vec<u64> {
        self.monitors
            .read()
            .unwrap()
            .get(&watched)
            .cloned()
            .unwrap_or_default()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VmState {
    pub pid: u64,
    pub frames: Vec<Frame>,
    pub stack: Vec<Value>,
    pub runtime: Runtime,
    pub mailbox: VecDeque<Value>,
    pub token_budget: usize,
    pub strictness_threshold: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Process {
    pub pid: u64,
    pub frames: Vec<Frame>,
    pub stack: Vec<Value>,
    pub runtime: Runtime,
    pub mailbox: VecDeque<Value>,
    pub token_budget: usize,
    pub strictness_threshold: f64,
    pub links: Vec<u64>,
    pub monitors: Vec<u64>,
}

pub struct Vm {
    pub registry: Registry,
    pub root_process: Option<Process>,
    pub root_rx: Option<tokio::sync::mpsc::UnboundedReceiver<Value>>,
}

impl Vm {
    pub fn new(code: &[Instr], host_tx: tokio::sync::mpsc::UnboundedSender<VmEvent>) -> Self {
        let registry = Registry::new(host_tx);
        let root_pid = 1;
        let (root_tx, root_rx) = tokio::sync::mpsc::unbounded_channel();
        registry.register(root_pid, root_tx);

        let root_frame = Frame {
            code: Arc::new(code.to_vec()),
            ip: 0,
            env: HashMap::new(),
            handlers: Vec::new(),
        };

        let root_process = Process {
            pid: root_pid,
            frames: vec![root_frame],
            stack: Vec::new(),
            runtime: Runtime::new(),
            mailbox: VecDeque::new(),
            token_budget: 1_000_000,
            strictness_threshold: 0.8,
            links: Vec::new(),
            monitors: Vec::new(),
        };

        Self {
            registry,
            root_process: Some(root_process),
            root_rx: Some(root_rx),
        }
    }

    pub fn snapshot(process: &Process, path: &str) -> std::io::Result<()> {
        let state = VmState {
            pid: process.pid,
            frames: process.frames.clone(),
            stack: process.stack.clone(),
            runtime: process.runtime.clone(),
            mailbox: process.mailbox.clone(),
            token_budget: process.token_budget,
            strictness_threshold: process.strictness_threshold,
        };
        let data = serde_json::to_string_pretty(&state)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    pub fn resume_from_disk(
        path: &str,
        host_tx: tokio::sync::mpsc::UnboundedSender<VmEvent>,
    ) -> std::io::Result<Self> {
        let data = std::fs::read_to_string(path)?;
        let state: VmState = serde_json::from_str(&data)?;

        let registry = Registry::new(host_tx);
        let (root_tx, root_rx) = tokio::sync::mpsc::unbounded_channel();
        registry.register(state.pid, root_tx);
        *registry.next_pid.write().unwrap() = state.pid + 1;

        let process = Process {
            pid: state.pid,
            frames: state.frames,
            stack: state.stack,
            runtime: state.runtime,
            mailbox: state.mailbox,
            token_budget: state.token_budget,
            strictness_threshold: state.strictness_threshold,
            links: Vec::new(),
            monitors: Vec::new(),
        };

        Ok(Self {
            registry,
            root_process: Some(process),
            root_rx: Some(root_rx),
        })
    }

    pub async fn start(self) {
        if let (Some(mut root), Some(rx)) = (self.root_process, self.root_rx) {
            let registry = self.registry.clone();
            tokio::spawn(async move {
                root.run_process(registry, rx).await;
            });
        }
    }
}

impl Process {
    fn exit_process(&self, registry: &Registry, result: Value) {
        let reason = if let Value::Str(_) = &result {
            result.clone()
        } else if result != Value::Null {
            result.clone()
        } else {
            Value::Str(std::sync::Arc::new("normal".to_string()))
        };
        for linked_pid in registry.get_links(self.pid) {
            let mut map = indexmap::IndexMap::new();
            map.insert(
                "type".to_string(),
                Value::Str(std::sync::Arc::new("EXIT".to_string())),
            );
            map.insert(
                "pid".to_string(),
                Value::Pid {
                    node_id: "local".to_string(),
                    local_pid: self.pid,
                },
            );
            map.insert("reason".to_string(), reason.clone());
            registry.send(linked_pid, Value::Map(std::sync::Arc::new(map)));
        }
        for monitor_pid in registry.get_monitors(self.pid) {
            let mut map = indexmap::IndexMap::new();
            map.insert(
                "type".to_string(),
                Value::Str(std::sync::Arc::new("DOWN".to_string())),
            );
            map.insert(
                "pid".to_string(),
                Value::Pid {
                    node_id: "local".to_string(),
                    local_pid: self.pid,
                },
            );
            map.insert("reason".to_string(), reason.clone());
            registry.send(monitor_pid, Value::Map(std::sync::Arc::new(map)));
        }
        registry.unregister(self.pid);
        let _ = registry.host_tx.send(VmEvent::Complete {
            pid: self.pid,
            result,
        });
    }

    #[async_recursion::async_recursion]
    pub async fn run_process(
        &mut self,
        registry: Registry,
        mut receiver: tokio::sync::mpsc::UnboundedReceiver<Value>,
    ) {
        let mut steps_left = 1000;

        loop {
            if self.token_budget == 0 {
                let _ = registry.host_tx.send(VmEvent::Error {
                    pid: self.pid,
                    error: "Runtime Error: TokenExhaustionError - Process ran out of gas"
                        .to_string(),
                });
                return;
            }

            if steps_left == 0 {
                tokio::task::yield_now().await;
                steps_left = 1000;
            }
            steps_left -= 1;
            self.token_budget = self.token_budget.saturating_sub(1);

            if self.frames.is_empty() {
                let ret_val = self.stack.pop().unwrap_or(Value::Null);
                self.exit_process(&registry, ret_val);
                return;
            }

            let frame_idx = self.frames.len() - 1;
            let frame = &mut self.frames[frame_idx];

            if frame.ip >= frame.code.len() {
                if self.frames.len() == 1 {
                    let ret_val = self.stack.pop().unwrap_or(Value::Null);
                    self.exit_process(&registry, ret_val);
                    return;
                } else {
                    self.frames.pop();
                    if let Some(caller) = self.frames.last() {
                        self.runtime.env = caller.env.clone();
                    }
                    self.stack.push(Value::Null);
                    continue;
                }
            }

            let instr = frame.code[frame.ip].clone();
            frame.ip += 1;

            match instr {
                Instr::Spawn => {
                    let target = self.stack.pop().unwrap_or(Value::Null);
                    if let Value::Closure { code, ip, env, .. } = target {
                        let new_pid = registry.get_next_pid();

                        let mut new_process = Process {
                            pid: new_pid,
                            frames: vec![Frame {
                                code,
                                ip,
                                env: env.clone(),
                                handlers: Vec::new(),
                            }],
                            stack: Vec::new(),
                            runtime: Runtime::new(),
                            mailbox: VecDeque::new(),
                            token_budget: 100_000, // Spawned tasks default gas limit
                            strictness_threshold: self.strictness_threshold,
                            links: Vec::new(),
                            monitors: Vec::new(),
                        };
                        new_process.runtime.env = env;

                        let (new_tx, new_rx) = tokio::sync::mpsc::unbounded_channel();
                        registry.register(new_pid, new_tx);

                        let reg_clone = registry.clone();
                        tokio::task::spawn(async move {
                            new_process.run_process(reg_clone, new_rx).await;
                        });

                        self.stack.push(Value::Pid {
                            node_id: "local".to_string(),
                            local_pid: new_pid,
                        });
                    } else {
                        self.stack.push(Value::Null);
                    }
                }
                Instr::SpawnRemote => {
                    let closure_val = self.stack.pop().unwrap_or(Value::Null);
                    let node_ip_val = self.stack.pop().unwrap_or(Value::Null);

                    if let (Value::Str(node_id), Value::Closure { .. }) =
                        (&node_ip_val, &closure_val)
                    {
                        let mut success = false;
                        if let Some(sb) = &self.runtime.switchboard {
                            if sb.send_remote(node_id, 0, closure_val.clone()).is_ok() {
                                success = true;
                                self.stack.push(Value::Bool(true));
                            }
                        }
                        if !success {
                            println!("[VM] Warning: SpawnRemote to {} failed.", node_id);
                            self.stack.push(Value::Bool(false));
                        }
                    } else {
                        self.stack.push(Value::Null);
                    }
                }
                Instr::Send => {
                    let msg = self.stack.pop().unwrap_or(Value::Null);
                    let pid_val = self.stack.pop().unwrap_or(Value::Null);

                    if let Value::Pid { node_id, local_pid } = pid_val {
                        let mut found = false;
                        if node_id != "local" {
                            if let Some(sb) = &self.runtime.switchboard {
                                if sb.send_remote(&node_id, local_pid, msg.clone()).is_ok() {
                                    found = true;
                                }
                            }
                        } else if local_pid == self.pid {
                            self.mailbox.push_back(msg);
                            found = true;
                        } else {
                            found = registry.send(local_pid, msg);
                        }
                        self.stack.push(Value::Bool(found));
                    } else {
                        self.stack.push(Value::Bool(false));
                    }
                }
                Instr::Receive => {
                    while let Ok(msg) = receiver.try_recv() {
                        self.mailbox.push_back(msg);
                    }
                    if let Some(msg) = self.mailbox.pop_front() {
                        self.stack.push(msg);
                    } else if let Some(msg) = receiver.recv().await {
                        self.stack.push(msg);
                    } else {
                        self.stack.push(Value::Null);
                    }
                }
                Instr::Link => {
                    let pid_val = self.stack.pop().unwrap_or(Value::Null);
                    if let Value::Pid { node_id, local_pid } = pid_val {
                        if node_id == "local" && !self.links.contains(&local_pid) {
                            self.links.push(local_pid);
                            registry.add_link(local_pid, self.pid);
                            registry.add_link(self.pid, local_pid);
                        }
                        self.stack.push(Value::Bool(true));
                    } else {
                        self.stack.push(Value::Bool(false));
                    }
                }
                Instr::Monitor => {
                    let pid_val = self.stack.pop().unwrap_or(Value::Null);
                    if let Value::Pid { node_id, local_pid } = pid_val {
                        if node_id == "local" {
                            registry.add_monitor(local_pid, self.pid);
                        }
                        self.stack.push(Value::Bool(true));
                    } else {
                        self.stack.push(Value::Bool(false));
                    }
                }
                Instr::Confidence => {
                    let v = self.stack.pop().unwrap_or(Value::Null);
                    match v {
                        Value::Uncertain(_, p) => self.stack.push(Value::Num(p)),
                        _ => self.stack.push(Value::Num(1.0)), // Certainty
                    }
                }
                Instr::Suspend => {
                    self.frames[frame_idx].env = self.runtime.env.clone();
                    let state = VmState {
                        pid: self.pid,
                        frames: self.frames.clone(),
                        stack: self.stack.clone(),
                        runtime: self.runtime.clone(),
                        mailbox: self.mailbox.clone(),
                        token_budget: self.token_budget,
                        strictness_threshold: self.strictness_threshold,
                    };

                    // Orthogonal Persistence: Flush active memory to local WAL
                    let _ = Vm::snapshot(self, ".turn_heap.json");

                    let (tx, rx) = tokio::sync::oneshot::channel();
                    let _ = registry.host_tx.send(VmEvent::Suspend {
                        pid: self.pid,
                        tool_name: "sys_suspend".to_string(),
                        arg: Value::Null,
                        resume_tx: tx,
                        continuation: Box::new(Some(state)),
                    });
                    if let Ok(res) = rx.await {
                        self.stack.push(res);
                    } else {
                        self.stack.push(Value::Null);
                    }
                }
                Instr::Infer(ty, tool_count) => {
                    let mut tools = Vec::new();
                    for _ in 0..tool_count {
                        tools.push(self.stack.pop().unwrap_or(Value::Null));
                    }
                    tools.reverse(); // stack pops in reverse order

                    let prompt_val = self.stack.pop().unwrap_or(Value::Null);
                    // Resolve named struct placeholders (Struct("Name", {}))
                    // against runtime-registered struct definitions before prompting LLM.
                    let resolved_ty = match ty {
                        Type::Struct(name, fields) if fields.is_empty() => {
                            if let Some(known_fields) = self.runtime.structs.get(&name) {
                                Type::Struct(name, known_fields.clone())
                            } else {
                                Type::Struct(name, fields)
                            }
                        }
                        other => other,
                    };
                    let ty_str =
                        serde_json::to_string(&resolved_ty).unwrap_or_else(|_| "{}".to_string());

                    // Semantic Auto-Recall (Phase 3)
                    // Take the user's prompt string, generate a vector representation behind the scenes,
                    // query the HNSW Semantic Graph for the Top 3 most semantically similar memories,
                    // and prepend them into the Token Budget context payload automatically.
                    let mut auto_context = Vec::new();
                    if let Value::Str(prompt_str) = &prompt_val {
                        if let Some(query_emb) = crate::llm_tools::get_embedding(prompt_str) {
                            let semantic_matches = self.runtime.memory.search(&query_emb, 3);
                            for memory_val in semantic_matches {
                                auto_context.push(memory_val);
                            }
                            // Bill tokens for the vector similarity operation
                            if self.token_budget >= 50 {
                                self.token_budget -= 50;
                            }
                        }
                    }

                    // Combine Auto-Recall + Explicit Context stack
                    let mut combined_context = auto_context;
                    combined_context.extend(self.runtime.context.to_values());

                    let mut map = IndexMap::new();
                    map.insert("prompt".to_string(), prompt_val);
                    map.insert(
                        "schema".to_string(),
                        Value::Str(std::sync::Arc::new(ty_str)),
                    );
                    map.insert(
                        "context".to_string(),
                        Value::List(std::sync::Arc::new(combined_context)),
                    );
                    if tool_count > 0 {
                        map.insert("tools".to_string(), Value::List(std::sync::Arc::new(tools)));
                    }

                    self.frames[frame_idx].env = self.runtime.env.clone();
                    let state = VmState {
                        pid: self.pid,
                        frames: self.frames.clone(),
                        stack: self.stack.clone(),
                        runtime: self.runtime.clone(),
                        mailbox: self.mailbox.clone(),
                        token_budget: self.token_budget,
                        strictness_threshold: self.strictness_threshold,
                    };
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    let _ = registry.host_tx.send(VmEvent::Suspend {
                        pid: self.pid,
                        tool_name: "llm_infer".to_string(),
                        arg: Value::Map(std::sync::Arc::new(map)),
                        resume_tx: tx,
                        continuation: Box::new(Some(state)),
                    });

                    if let Ok(res) = rx.await {
                        self.stack.push(res);
                    } else {
                        self.stack.push(Value::Null);
                    }
                }
                Instr::DefineStruct(name, fields) => {
                    self.runtime.register_struct(name, fields);
                }
                Instr::PushNull => self.stack.push(Value::Null),
                Instr::PushTrue => self.stack.push(Value::Bool(true)),
                Instr::PushFalse => self.stack.push(Value::Bool(false)),
                Instr::PushNum(n) => self.stack.push(Value::Num(n)),
                Instr::PushStr(s) => self.stack.push(Value::Str(std::sync::Arc::new(s))),
                Instr::Pop => {
                    self.stack.pop();
                }
                Instr::Load(name) => match self.runtime.get_env(&name) {
                    Some(v) => self.stack.push(v),
                    None => self.stack.push(Value::Null),
                },
                Instr::Store(name) => {
                    let val = self.stack.pop().unwrap_or(Value::Null);
                    self.runtime.push_env(name, val);
                }
                Instr::Recall => {
                    let key = self.stack.pop().unwrap_or(Value::Null);
                    let val = self.runtime.recall(&key);
                    self.stack.push(val);
                }
                Instr::Remember => {
                    let val = self.stack.pop().unwrap_or(Value::Null);
                    let key = self.stack.pop().unwrap_or(Value::Null);

                    // Auto-Embedding Logic for Semantic RAM
                    // If the user did not provide an explicit embedding inside a Struct/Map,
                    // the Virtual Machine generates one implicitly via Azure OpenAI
                    let (id, mut emb) = match &key {
                        Value::Map(m) | Value::Struct(_, m) => {
                            let k_id = m
                                .get("id")
                                .and_then(|v| crate::runtime::value_to_key(v).ok())
                                .unwrap_or_else(|| "unnamed".to_string());
                            let k_emb = match m.get("embedding") {
                                Some(Value::Vec(v)) => Some(v.clone()),
                                _ => None,
                            };
                            (k_id, k_emb)
                        }
                        _ => (crate::runtime::value_to_key(&key).unwrap_or_default(), None),
                    };

                    if emb.is_none() && !id.is_empty() {
                        // Generate implicit vector coordinates representing the semantic memory
                        emb = crate::llm_tools::get_embedding(&id).map(std::sync::Arc::new);
                        // Bill the process for the background LLM operation
                        if self.token_budget >= 50 {
                            self.token_budget -= 50;
                        }
                    }

                    self.runtime
                        .memory
                        .insert(id, emb.map(|e| (*e).clone()), val);
                }
                Instr::Add => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) {
                        self.exit_process(
                            &registry,
                            Value::Str(std::sync::Arc::new(
                                "PrivilegeViolation: Opaque capabilities cannot be observed"
                                    .to_string(),
                            )),
                        );
                        return;
                    }
                    if let Value::Uncertain(_, p) = a {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    if let Value::Uncertain(_, p) = b {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    self.stack.push(add_values(&a, &b));
                }
                Instr::Sub => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) {
                        self.exit_process(
                            &registry,
                            Value::Str(std::sync::Arc::new(
                                "PrivilegeViolation: Opaque capabilities cannot be observed"
                                    .to_string(),
                            )),
                        );
                        return;
                    }
                    if let Value::Uncertain(_, p) = a {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    if let Value::Uncertain(_, p) = b {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    self.stack.push(sub_values(&a, &b));
                }
                Instr::Mul => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) {
                        self.exit_process(
                            &registry,
                            Value::Str(std::sync::Arc::new(
                                "PrivilegeViolation: Opaque capabilities cannot be observed"
                                    .to_string(),
                            )),
                        );
                        return;
                    }
                    if let Value::Uncertain(_, p) = a {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    if let Value::Uncertain(_, p) = b {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    self.stack.push(mul_values(&a, &b));
                }
                Instr::Div => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) {
                        self.exit_process(
                            &registry,
                            Value::Str(std::sync::Arc::new(
                                "PrivilegeViolation: Opaque capabilities cannot be observed"
                                    .to_string(),
                            )),
                        );
                        return;
                    }
                    if let Value::Uncertain(_, p) = a {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    if let Value::Uncertain(_, p) = b {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    self.stack.push(div_values(&a, &b));
                }
                Instr::Eq => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) {
                        self.exit_process(
                            &registry,
                            Value::Str(std::sync::Arc::new(
                                "PrivilegeViolation: Opaque capabilities cannot be observed"
                                    .to_string(),
                            )),
                        );
                        return;
                    }
                    if let Value::Uncertain(_, p) = a {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    if let Value::Uncertain(_, p) = b {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    self.stack.push(eq_values(&a, &b));
                }
                Instr::Ne => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) {
                        self.exit_process(
                            &registry,
                            Value::Str(std::sync::Arc::new(
                                "PrivilegeViolation: Opaque capabilities cannot be observed"
                                    .to_string(),
                            )),
                        );
                        return;
                    }
                    if let Value::Uncertain(_, p) = a {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    if let Value::Uncertain(_, p) = b {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    self.stack.push(ne_values(&a, &b));
                }
                Instr::Lt => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) {
                        self.exit_process(
                            &registry,
                            Value::Str(std::sync::Arc::new(
                                "PrivilegeViolation: Opaque capabilities cannot be observed"
                                    .to_string(),
                            )),
                        );
                        return;
                    }
                    if let Value::Uncertain(_, p) = a {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    if let Value::Uncertain(_, p) = b {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    self.stack.push(lt_values(&a, &b));
                }
                Instr::Gt => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) {
                        self.exit_process(
                            &registry,
                            Value::Str(std::sync::Arc::new(
                                "PrivilegeViolation: Opaque capabilities cannot be observed"
                                    .to_string(),
                            )),
                        );
                        return;
                    }
                    if let Value::Uncertain(_, p) = a {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    if let Value::Uncertain(_, p) = b {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    self.stack.push(gt_values(&a, &b));
                }
                Instr::Le => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) {
                        self.exit_process(
                            &registry,
                            Value::Str(std::sync::Arc::new(
                                "PrivilegeViolation: Opaque capabilities cannot be observed"
                                    .to_string(),
                            )),
                        );
                        return;
                    }
                    if let Value::Uncertain(_, p) = a {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    if let Value::Uncertain(_, p) = b {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    self.stack.push(le_values(&a, &b));
                }
                Instr::Ge => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) {
                        self.exit_process(
                            &registry,
                            Value::Str(std::sync::Arc::new(
                                "PrivilegeViolation: Opaque capabilities cannot be observed"
                                    .to_string(),
                            )),
                        );
                        return;
                    }
                    if let Value::Uncertain(_, p) = a {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    if let Value::Uncertain(_, p) = b {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    self.stack.push(ge_values(&a, &b));
                }
                Instr::Not => {
                    let v = self.stack.pop().unwrap_or(Value::Null);
                    if matches!(v, Value::Cap(_)) {
                        self.exit_process(
                            &registry,
                            Value::Str(std::sync::Arc::new(
                                "PrivilegeViolation: Opaque capabilities cannot be observed"
                                    .to_string(),
                            )),
                        );
                        return;
                    }
                    if let Value::Uncertain(_, p) = v {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    self.stack.push(not_value(&v));
                }
                Instr::And => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) {
                        self.exit_process(
                            &registry,
                            Value::Str(std::sync::Arc::new(
                                "PrivilegeViolation: Opaque capabilities cannot be observed"
                                    .to_string(),
                            )),
                        );
                        return;
                    }
                    if let Value::Uncertain(_, p) = a {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    if let Value::Uncertain(_, p) = b {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    self.stack.push(and_values(&a, &b));
                }
                Instr::Or => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) {
                        self.exit_process(
                            &registry,
                            Value::Str(std::sync::Arc::new(
                                "PrivilegeViolation: Opaque capabilities cannot be observed"
                                    .to_string(),
                            )),
                        );
                        return;
                    }
                    if let Value::Uncertain(_, p) = a {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    if let Value::Uncertain(_, p) = b {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    self.stack.push(or_values(&a, &b));
                }
                Instr::Jump(target) => {
                    self.frames[frame_idx].ip = target as usize;
                }
                Instr::JumpIfFalse(target) => {
                    let v = self.stack.pop().unwrap_or(Value::Null);
                    if matches!(v, Value::Cap(_)) {
                        self.exit_process(
                            &registry,
                            Value::Str(std::sync::Arc::new(
                                "PrivilegeViolation: Opaque capabilities cannot be branched upon"
                                    .to_string(),
                            )),
                        );
                        return;
                    }
                    if let Value::Uncertain(_, p) = v {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    if v.is_falsy() {
                        self.frames[frame_idx].ip = target as usize;
                    }
                }
                Instr::JumpIfTrue(target) => {
                    let v = self.stack.pop().unwrap_or(Value::Null);
                    if matches!(v, Value::Cap(_)) {
                        self.exit_process(
                            &registry,
                            Value::Str(std::sync::Arc::new(
                                "PrivilegeViolation: Opaque capabilities cannot be branched upon"
                                    .to_string(),
                            )),
                        );
                        return;
                    }
                    if let Value::Uncertain(_, p) = v {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    if v.is_truthy() {
                        self.frames[frame_idx].ip = target as usize;
                    }
                }
                Instr::ContextAppend => {
                    let v = self.stack.pop().unwrap_or(Value::Null);
                    if matches!(v, Value::Cap(_)) {
                        self.exit_process(&registry, Value::Str(std::sync::Arc::new("PrivilegeViolation: Opaque capabilities cannot be appended to Agent context".to_string())));
                        return;
                    }
                    if let Value::Uncertain(_, p) = v {
                        if p < self.strictness_threshold {
                            self.exit_process(
                                &registry,
                                Value::Str(std::sync::Arc::new(format!(
                                    "ExecutionTrap: Low confidence {} below threshold {}",
                                    p, self.strictness_threshold
                                ))),
                            );
                            return;
                        }
                    }
                    let _ = self.runtime.append_context(v);
                }
                Instr::EnterTurn(after_addr) => {
                    let current_ip = self.frames[frame_idx].ip;
                    self.frames[frame_idx].ip = after_addr as usize;
                    let code = self.frames[frame_idx].code.clone();
                    let env = self.runtime.env.clone();
                    self.frames.push(Frame {
                        code,
                        ip: current_ip,
                        env,
                        handlers: Vec::new(),
                    });
                }
                Instr::MakeOk => {
                    let v = self.stack.pop().unwrap_or(Value::Null);
                    let mut map = IndexMap::new();
                    map.insert("ok".to_string(), v);
                    self.stack.push(Value::Struct(
                        std::sync::Arc::new("Result".to_string()),
                        std::sync::Arc::new(map),
                    ));
                }
                Instr::MakeErr => {
                    let v = self.stack.pop().unwrap_or(Value::Null);
                    let mut map = IndexMap::new();
                    map.insert("err".to_string(), v);
                    self.stack.push(Value::Struct(
                        std::sync::Arc::new("Result".to_string()),
                        std::sync::Arc::new(map),
                    ));
                }
                Instr::MatchResult(target) => {
                    let v = self.stack.pop().unwrap_or(Value::Null);
                    match v {
                        Value::Struct(name, fields) if name.as_str() == "Result" => {
                            if let Some(err_val) = fields.get("err") {
                                self.stack.push(err_val.clone());
                                self.frames[frame_idx].ip = target as usize;
                            } else if let Some(ok_val) = fields.get("ok") {
                                self.stack.push(ok_val.clone());
                            } else {
                                self.stack.push(Value::Null); // Malformed Result
                            }
                        }
                        _ => {
                            // If it's not a Result struct, treat it as an Error for safety, or wrap as Ok.
                            // For strictness, let's say matching on non-Result acts as Ok(v) to fallthrough.
                            self.stack.push(v);
                        }
                    }
                }
                Instr::CallMethod(name) => {
                    let arg = self.stack.pop().unwrap_or(Value::Null);
                    let target = self.stack.pop().unwrap_or(Value::Null);

                    let (tool_val, final_arg) = if let Some(func) = match &target {
                        Value::Map(m) | Value::Struct(_, m) => m.get(&name).cloned(),
                        _ => None,
                    } {
                        (func, arg)
                    } else if let Some(func) = self.runtime.get_env(&name) {
                        let final_arg = if arg.is_falsy() { target } else { arg };
                        (func, final_arg)
                    } else {
                        (Value::Null, Value::Null)
                    };

                    match tool_val {
                        Value::Str(name) => {
                            self.frames[frame_idx].env = self.runtime.env.clone();
                            let state = VmState {
                                pid: self.pid,
                                frames: self.frames.clone(),
                                stack: self.stack.clone(),
                                runtime: self.runtime.clone(),
                                mailbox: self.mailbox.clone(),
                                token_budget: self.token_budget,
                                strictness_threshold: self.strictness_threshold,
                            };
                            let (tx, rx) = tokio::sync::oneshot::channel();
                            let _ = registry.host_tx.send(VmEvent::Suspend {
                                pid: self.pid,
                                tool_name: name.to_string(),
                                arg: final_arg,
                                resume_tx: tx,
                                continuation: Box::new(Some(state)),
                            });
                            if let Ok(res) = rx.await {
                                self.stack.push(res);
                            } else {
                                self.stack.push(Value::Null);
                            }
                        }
                        Value::Closure {
                            is_tool: _,
                            code,
                            ip,
                            env,
                            params,
                        } => {
                            let mut new_env = env.clone();
                            let mut mem_inserts = Vec::new();

                            if params.len() == 1 {
                                let name = &params[0].0;
                                match final_arg {
                                    Value::Map(m) => {
                                        if m.contains_key(name) {
                                            for (k, v) in m.iter() {
                                                mem_inserts.push((k.clone(), v.clone()));
                                                new_env.insert(k.clone(), v.clone());
                                            }
                                        } else {
                                            let wrapped = Value::Map(m);
                                            mem_inserts.push((name.clone(), wrapped.clone()));
                                            new_env.insert(name.clone(), wrapped);
                                        }
                                    }
                                    Value::Struct(struct_name, m) => {
                                        if m.contains_key(name) {
                                            for (k, v) in m.iter() {
                                                mem_inserts.push((k.clone(), v.clone()));
                                                new_env.insert(k.clone(), v.clone());
                                            }
                                        } else {
                                            let wrapped = Value::Struct(struct_name, m);
                                            mem_inserts.push((name.clone(), wrapped.clone()));
                                            new_env.insert(name.clone(), wrapped);
                                        }
                                    }
                                    other => {
                                        mem_inserts.push((name.clone(), other.clone()));
                                        new_env.insert(name.clone(), other);
                                    }
                                }
                            } else if let Value::Map(m) = final_arg {
                                for (k, v) in m.iter() {
                                    mem_inserts.push((k.clone(), v.clone()));
                                    new_env.insert(k.clone(), v.clone());
                                }
                            } else if let Value::Struct(_, m) = final_arg.clone() {
                                for (k, v) in m.iter() {
                                    mem_inserts.push((k.clone(), v.clone()));
                                    new_env.insert(k.clone(), v.clone());
                                }
                            } else if let Value::List(items) = final_arg {
                                for (i, item) in items.iter().enumerate() {
                                    if i < params.len() {
                                        mem_inserts.push((params[i].0.clone(), item.clone()));
                                        new_env.insert(params[i].0.clone(), item.clone());
                                    }
                                }
                            } else if !final_arg.is_falsy() {
                                mem_inserts.push(("arg".to_string(), final_arg.clone()));
                                new_env.insert("arg".to_string(), final_arg);
                            }

                            self.frames[frame_idx].env = self.runtime.env.clone();
                            self.runtime.env = new_env;
                            for (k, v) in mem_inserts {
                                self.runtime.memory.insert(k, None, v);
                            }

                            self.frames.push(Frame {
                                code,
                                ip,
                                env: self.runtime.env.clone(),
                                handlers: Vec::new(),
                            });
                        }
                        _ => self.stack.push(Value::Null),
                    }
                }
                Instr::CallTool => {
                    let arg = self.stack.pop().unwrap_or(Value::Null);
                    let tool_val = self.stack.pop().unwrap_or(Value::Null);

                    match tool_val {
                        Value::Str(name) => {
                            self.frames[frame_idx].env = self.runtime.env.clone();
                            let state = VmState {
                                pid: self.pid,
                                frames: self.frames.clone(),
                                stack: self.stack.clone(),
                                runtime: self.runtime.clone(),
                                mailbox: self.mailbox.clone(),
                                token_budget: self.token_budget,
                                strictness_threshold: self.strictness_threshold,
                            };
                            let (tx, rx) = tokio::sync::oneshot::channel();
                            let _ = registry.host_tx.send(VmEvent::Suspend {
                                pid: self.pid,
                                tool_name: name.to_string(),
                                arg,
                                resume_tx: tx,
                                continuation: Box::new(Some(state)),
                            });
                            if let Ok(res) = rx.await {
                                self.stack.push(res);
                            } else {
                                self.stack.push(Value::Null);
                            }
                            // Process continues natively without halting!
                        }
                        Value::Closure {
                            is_tool: _,
                            code,
                            ip,
                            env,
                            params,
                        } => {
                            let mut new_env = env.clone();
                            let mut mem_inserts = Vec::new();

                            if params.len() == 1 {
                                let name = &params[0].0;
                                match arg {
                                    Value::Map(m) => {
                                        if m.contains_key(name) {
                                            for (k, v) in m.iter() {
                                                mem_inserts.push((k.clone(), v.clone()));
                                                new_env.insert(k.clone(), v.clone());
                                            }
                                        } else {
                                            let wrapped = Value::Map(m);
                                            mem_inserts.push((name.clone(), wrapped.clone()));
                                            new_env.insert(name.clone(), wrapped);
                                        }
                                    }
                                    Value::Struct(struct_name, m) => {
                                        if m.contains_key(name) {
                                            for (k, v) in m.iter() {
                                                mem_inserts.push((k.clone(), v.clone()));
                                                new_env.insert(k.clone(), v.clone());
                                            }
                                        } else {
                                            let wrapped = Value::Struct(struct_name, m);
                                            mem_inserts.push((name.clone(), wrapped.clone()));
                                            new_env.insert(name.clone(), wrapped);
                                        }
                                    }
                                    other => {
                                        mem_inserts.push((name.clone(), other.clone()));
                                        new_env.insert(name.clone(), other);
                                    }
                                }
                            } else if let Value::Map(m) = arg {
                                for (k, v) in m.iter() {
                                    mem_inserts.push((k.clone(), v.clone()));
                                    new_env.insert(k.clone(), v.clone());
                                }
                            } else if let Value::Struct(_, m) = arg.clone() {
                                for (k, v) in m.iter() {
                                    mem_inserts.push((k.clone(), v.clone()));
                                    new_env.insert(k.clone(), v.clone());
                                }
                            } else if let Value::List(items) = arg {
                                for (i, item) in items.iter().enumerate() {
                                    if i < params.len() {
                                        mem_inserts.push((params[i].0.clone(), item.clone()));
                                        new_env.insert(params[i].0.clone(), item.clone());
                                    }
                                }
                            } else if !arg.is_falsy() {
                                mem_inserts.push(("arg".to_string(), arg.clone()));
                                new_env.insert("arg".to_string(), arg);
                            }

                            self.frames[frame_idx].env = self.runtime.env.clone();
                            self.runtime.env = new_env;
                            for (k, v) in mem_inserts {
                                self.runtime.memory.insert(k, None, v);
                            }

                            self.frames.push(Frame {
                                code,
                                ip,
                                env: self.runtime.env.clone(),
                                handlers: Vec::new(),
                            });
                        }
                        _ => self.stack.push(Value::Null),
                    }
                }
                Instr::Return => {
                    let ret_val = self.stack.pop().unwrap_or(Value::Null);
                    if self.frames.len() > 1 {
                        self.frames.pop();
                        if let Some(caller) = self.frames.last() {
                            self.runtime.env = caller.env.clone();
                        }
                        self.stack.push(ret_val);
                    } else {
                        self.exit_process(&registry, ret_val);
                        return;
                    }
                }
                Instr::MakeList(count) => {
                    let mut items = Vec::new();
                    for _ in 0..count {
                        items.push(self.stack.pop().unwrap_or(Value::Null));
                    }
                    items.reverse();
                    self.stack.push(Value::List(std::sync::Arc::new(items)));
                }
                Instr::MakeMap(count) => {
                    let mut map = IndexMap::new();
                    for _ in 0..count {
                        let val = self.stack.pop().unwrap_or(Value::Null);
                        let k_val = self.stack.pop().unwrap_or(Value::Null);
                        let k = match k_val {
                            Value::Str(s) => s.to_string(),
                            _ => k_val.to_string(),
                        };
                        map.insert(k, val);
                    }
                    self.stack.push(Value::Map(std::sync::Arc::new(map)));
                }
                Instr::MakeStruct(name, count) => {
                    let mut map = IndexMap::new();
                    for _ in 0..count {
                        let val = self.stack.pop().unwrap_or(Value::Null);
                        let k_val = self.stack.pop().unwrap_or(Value::Null);
                        let k = match k_val {
                            Value::Str(s) => s.to_string(),
                            _ => k_val.to_string(),
                        };
                        map.insert(k, val);
                    }
                    self.stack.push(Value::Struct(
                        std::sync::Arc::new(name),
                        std::sync::Arc::new(map),
                    ));
                }
                Instr::MakeVec(count) => {
                    let mut items = Vec::new();
                    for _ in 0..count {
                        let v = self.stack.pop().unwrap_or(Value::Null);
                        if let Value::Num(n) = v {
                            items.push(n);
                        } else {
                            // Runtime error: Vector elements must be numbers
                            items.push(0.0); // Fallback
                        }
                    }
                    items.reverse();
                    self.stack.push(Value::Vec(std::sync::Arc::new(items)));
                }
                Instr::Similarity => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    if let (Value::Vec(v1), Value::Vec(v2)) = (a, b) {
                        self.stack.push(Value::Num(cosine_similarity(&v1, &v2)));
                    } else {
                        self.stack.push(Value::Null);
                    }
                }
                Instr::Index => {
                    let idx = self.stack.pop().unwrap_or(Value::Null);
                    let tgt = self.stack.pop().unwrap_or(Value::Null);
                    let res = match tgt {
                        Value::List(l) => {
                            if let Value::Num(n) = idx {
                                l.get(n as usize).cloned().unwrap_or(Value::Null)
                            } else {
                                Value::Null
                            }
                        }
                        Value::Map(m) | Value::Struct(_, m) => {
                            let k = match idx {
                                Value::Str(s) => s.to_string(),
                                Value::Num(n) => n.to_string(),
                                _ => "".to_string(),
                            };
                            m.get(&k).cloned().unwrap_or(Value::Null)
                        }
                        Value::Uncertain(inner, _) => {
                            // Auto-unwrap uncertain for property access
                            match inner.as_ref() {
                                Value::Map(m) | Value::Struct(_, m) => {
                                    let k = match idx {
                                        Value::Str(s) => s.to_string(),
                                        Value::Num(n) => n.to_string(),
                                        _ => "".to_string(),
                                    };
                                    m.get(&k).cloned().unwrap_or(Value::Null)
                                }
                                _ => Value::Null,
                            }
                        }
                        _ => Value::Null,
                    };
                    self.stack.push(res);
                }
                Instr::MakeTurn(offset, is_tool, params) => {
                    let code = self.frames[frame_idx].code.clone();
                    let env = self.runtime.env.clone();
                    self.stack.push(Value::Closure {
                        is_tool,
                        code,
                        ip: offset as usize,
                        env,
                        params: params.clone(),
                    });
                }
                Instr::LoadModule => {
                    let p_val = self.stack.pop().unwrap_or(Value::Null);
                    let path = match p_val {
                        Value::Str(s) => s.clone(),
                        _ => std::sync::Arc::new("".to_string()),
                    };
                    self.frames[frame_idx].env = self.runtime.env.clone();
                    let state = VmState {
                        pid: self.pid,
                        frames: self.frames.clone(),
                        stack: self.stack.clone(),
                        runtime: self.runtime.clone(),
                        mailbox: self.mailbox.clone(),
                        token_budget: self.token_budget,
                        strictness_threshold: self.strictness_threshold,
                    };
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    let _ = registry.host_tx.send(VmEvent::Suspend {
                        pid: self.pid,
                        tool_name: "sys_import".to_string(),
                        arg: Value::Str(path),
                        resume_tx: tx,
                        continuation: Box::new(Some(state)),
                    });
                    if let Ok(res) = rx.await {
                        self.stack.push(res);
                    } else {
                        self.stack.push(Value::Null);
                    }
                }
                Instr::CheckType(ref ty) => {
                    let val = self.stack.last().unwrap_or(&Value::Null);
                    if !self.check_value_type(ty, val) {
                        let err = Value::Str(std::sync::Arc::new(format!(
                            "Runtime Type Error: Expected {:?}, got {:?}",
                            ty, val
                        )));
                        // Unwind
                        loop {
                            if self.frames.is_empty() {
                                self.exit_process(&registry, err);
                                return;
                            }
                            let f_idx = self.frames.len() - 1;
                            if let Some(off) = self.frames[f_idx].handlers.pop() {
                                self.frames[f_idx].ip = off as usize;
                                self.stack.push(err);
                                break;
                            } else {
                                self.frames.pop();
                                if let Some(c) = self.frames.last() {
                                    self.runtime.env = c.env.clone();
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn check_value_type(&self, ty: &Type, val: &Value) -> bool {
        // Unwrap Uncertain
        if let Value::Uncertain(inner, _) = val {
            return self.check_value_type(ty, inner);
        }
        match (ty, val) {
            (Type::Num, Value::Num(_)) => true,
            (Type::Str, Value::Str(_)) => true,
            (Type::Bool, Value::Bool(_)) => true,
            (Type::Bool, Value::Null) => false,
            (Type::List(inner), Value::List(items)) => {
                if **inner == Type::Any {
                    return true;
                }
                for item in items.iter() {
                    if !self.check_value_type(inner, item) {
                        return false;
                    }
                }
                true
            }
            (Type::Map(_k, inner), Value::Map(map)) => {
                if **inner == Type::Any {
                    return true;
                }
                for (_, val) in map.iter() {
                    if !self.check_value_type(inner, val) {
                        return false;
                    }
                }
                true
            }
            (Type::Struct(name, fields), Value::Struct(val_name, val_fields)) => {
                if name.as_str() != val_name.as_str() {
                    return false;
                }
                for (field_name, field_ty) in fields {
                    if let Some(val) = val_fields.get(field_name) {
                        if !self.check_value_type(field_ty, val) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                true
            }
            (Type::Function(_arg_ty, _ret_ty), Value::Closure { .. }) => true,
            (Type::Pid, Value::Pid { .. }) => true,
            (Type::Any, _) => true,
            (Type::Void, Value::Null) => true,
            _ => false,
        }
    }

    // Helper to push to CURRENT running process stack (root or first available)
    // Used by tests mainly.
}

impl Vm {
    pub fn push(&mut self, v: Value) {
        if let Some(p) = self.root_process.as_mut() {
            p.stack.push(v);
        }
    }

    pub fn pop(&mut self) -> Option<Value> {
        self.root_process.as_mut().and_then(|p| p.stack.pop())
    }

    pub fn peek(&self) -> Option<&Value> {
        self.root_process.as_ref().and_then(|p| p.stack.last())
    }
}

fn add_values(a: &Value, b: &Value) -> Value {
    // Unwrap Uncertain first for cleaner logic if both uncertain?
    // Recursive match.
    match (a, b) {
        (Value::Uncertain(v1, p1), Value::Uncertain(v2, p2)) => {
            let res = add_values(v1, v2);
            // If res is Uncertain, combine probabilities?
            // add_values(v1, v2) might return Uncertain if v1/v2 are nested.
            // Let's assume flattened.
            if let Value::Uncertain(inner, p3) = res {
                Value::Uncertain(inner, p1 * p2 * p3)
            } else {
                Value::Uncertain(Box::new(res), p1 * p2)
            }
        }
        (Value::Uncertain(v, p), other) => {
            let res = add_values(v, other);
            if let Value::Uncertain(inner, p2) = res {
                Value::Uncertain(inner, p * p2)
            } else {
                Value::Uncertain(Box::new(res), *p)
            }
        }
        (other, Value::Uncertain(v, p)) => {
            let res = add_values(other, v);
            if let Value::Uncertain(inner, p2) = res {
                Value::Uncertain(inner, p * p2)
            } else {
                Value::Uncertain(Box::new(res), *p)
            }
        }
        (Value::Num(x), Value::Num(y)) => Value::Num(x + y),
        (Value::Vec(v1), Value::Vec(v2)) => {
            if v1.len() != v2.len() {
                return Value::Null;
            }
            let sum: Vec<f64> = v1.iter().zip(v2.iter()).map(|(x, y)| x + y).collect();
            Value::Vec(std::sync::Arc::new(sum))
        }
        (Value::List(l1), Value::List(l2)) => {
            let mut new_list = (**l1).clone();
            new_list.extend(l2.iter().cloned());
            Value::List(std::sync::Arc::new(new_list))
        }
        (Value::Map(m1), Value::Map(m2)) => {
            let mut new_map = (**m1).clone();
            for (k, v) in m2.iter() {
                new_map.insert(k.clone(), v.clone());
            }
            Value::Map(std::sync::Arc::new(new_map))
        }
        (Value::Struct(name1, m1), Value::Struct(name2, m2)) if name1 == name2 => {
            let mut new_map = (**m1).clone();
            for (k, v) in m2.iter() {
                new_map.insert(k.clone(), v.clone());
            }
            Value::Struct(name1.clone(), std::sync::Arc::new(new_map))
        }
        _ => Value::Str(std::sync::Arc::new(format!("{}{}", a, b))),
    }
}

fn mul_values(a: &Value, b: &Value) -> Value {
    match (a, b) {
        (Value::Uncertain(v1, p1), Value::Uncertain(v2, p2)) => {
            let res = mul_values(v1, v2);
            if let Value::Uncertain(inner, p3) = res {
                Value::Uncertain(inner, p1 * p2 * p3)
            } else {
                Value::Uncertain(Box::new(res), p1 * p2)
            }
        }
        (Value::Uncertain(v, p), other) => {
            let res = mul_values(v, other);
            if let Value::Uncertain(inner, p2) = res {
                Value::Uncertain(inner, p * p2)
            } else {
                Value::Uncertain(Box::new(res), *p)
            }
        }
        (other, Value::Uncertain(v, p)) => {
            let res = mul_values(other, v);
            if let Value::Uncertain(inner, p2) = res {
                Value::Uncertain(inner, p * p2)
            } else {
                Value::Uncertain(Box::new(res), *p)
            }
        }
        (Value::Num(x), Value::Num(y)) => Value::Num(x * y),
        (Value::Vec(v), Value::Num(x)) | (Value::Num(x), Value::Vec(v)) => {
            let res: Vec<f64> = v.iter().map(|n| n * x).collect();
            Value::Vec(std::sync::Arc::new(res))
        }
        (Value::Vec(v1), Value::Vec(v2)) => {
            if v1.len() != v2.len() {
                return Value::Null;
            }
            let dot: f64 = v1.iter().zip(v2.iter()).map(|(x, y)| x * y).sum();
            Value::Num(dot)
        }
        _ => Value::Null,
    }
}

fn cosine_similarity(v1: &[f64], v2: &[f64]) -> f64 {
    if v1.len() != v2.len() {
        return 0.0;
    }
    let dot: f64 = v1.iter().zip(v2.iter()).map(|(x, y)| x * y).sum();
    let mag1: f64 = v1.iter().map(|x| x * x).sum::<f64>().sqrt();
    let mag2: f64 = v2.iter().map(|x| x * x).sum::<f64>().sqrt();
    if mag1 == 0.0 || mag2 == 0.0 {
        return 0.0;
    }
    dot / (mag1 * mag2)
}

fn eq_values(a: &Value, b: &Value) -> Value {
    match (a, b) {
        (Value::Uncertain(v1, p1), Value::Uncertain(v2, p2)) => {
            let res = eq_values(v1, v2);
            if let Value::Uncertain(inner, p3) = res {
                Value::Uncertain(inner, p1 * p2 * p3)
            } else {
                Value::Uncertain(Box::new(res), p1 * p2)
            }
        }
        (Value::Uncertain(v, p), other) => {
            let res = eq_values(v, other);
            if let Value::Uncertain(inner, p2) = res {
                Value::Uncertain(inner, p * p2)
            } else {
                Value::Uncertain(Box::new(res), *p)
            }
        }
        (other, Value::Uncertain(v, p)) => {
            let res = eq_values(other, v);
            if let Value::Uncertain(inner, p2) = res {
                Value::Uncertain(inner, p * p2)
            } else {
                Value::Uncertain(Box::new(res), *p)
            }
        }
        _ => Value::Bool(a == b),
    }
}

fn ne_values(a: &Value, b: &Value) -> Value {
    not_value(&eq_values(a, b))
}

fn not_value(v: &Value) -> Value {
    match v {
        Value::Uncertain(inner, p) => {
            let res = not_value(inner);
            if let Value::Uncertain(i, p2) = res {
                Value::Uncertain(i, p * p2)
            } else {
                Value::Uncertain(Box::new(res), *p)
            }
        }
        _ => Value::Bool(v.is_falsy()), // Not Falsy = Truthy
    }
}

fn and_values(a: &Value, b: &Value) -> Value {
    match (a, b) {
        (Value::Uncertain(v1, p1), Value::Uncertain(v2, p2)) => {
            let res = and_values(v1, v2);
            if let Value::Uncertain(inner, p3) = res {
                Value::Uncertain(inner, p1 * p2 * p3)
            } else {
                Value::Uncertain(Box::new(res), p1 * p2)
            }
        }
        (Value::Uncertain(v, p), other) => {
            let res = and_values(v, other);
            if let Value::Uncertain(inner, p2) = res {
                Value::Uncertain(inner, p * p2)
            } else {
                Value::Uncertain(Box::new(res), *p)
            }
        }
        (other, Value::Uncertain(v, p)) => {
            let res = and_values(other, v);
            if let Value::Uncertain(inner, p2) = res {
                Value::Uncertain(inner, p * p2)
            } else {
                Value::Uncertain(Box::new(res), *p)
            }
        }
        _ => Value::Bool(a.is_truthy() && b.is_truthy()),
    }
}

fn or_values(a: &Value, b: &Value) -> Value {
    match (a, b) {
        (Value::Uncertain(v1, p1), Value::Uncertain(v2, p2)) => {
            let res = or_values(v1, v2);
            if let Value::Uncertain(inner, p3) = res {
                // For OR, probability is complex: P(A or B) = P(A) + P(B) - P(A)P(B)
                // But here we propagate confidence of the *result* value being correct?
                // Or the confidence of the path?
                // Let's stick to simple product for now, representing "confidence in this computation".
                Value::Uncertain(inner, p1 * p2 * p3)
            } else {
                Value::Uncertain(Box::new(res), p1 * p2)
            }
        }
        (Value::Uncertain(v, p), other) => {
            let res = or_values(v, other);
            Value::Uncertain(Box::new(res), *p)
        }
        (other, Value::Uncertain(v, p)) => {
            let res = or_values(other, v);
            Value::Uncertain(Box::new(res), *p)
        }
        _ => Value::Bool(a.is_truthy() || b.is_truthy()),
    }
}

fn sub_values(a: &Value, b: &Value) -> Value {
    match (a, b) {
        (Value::Uncertain(v1, p1), Value::Uncertain(v2, p2)) => {
            let res = sub_values(v1, v2);
            if let Value::Uncertain(inner, p3) = res {
                Value::Uncertain(inner, p1 * p2 * p3)
            } else {
                Value::Uncertain(Box::new(res), p1 * p2)
            }
        }
        (Value::Uncertain(v, p), other) => {
            let res = sub_values(v, other);
            if let Value::Uncertain(inner, p2) = res {
                Value::Uncertain(inner, p * p2)
            } else {
                Value::Uncertain(Box::new(res), *p)
            }
        }
        (other, Value::Uncertain(v, p)) => {
            let res = sub_values(other, v);
            if let Value::Uncertain(inner, p2) = res {
                Value::Uncertain(inner, p * p2)
            } else {
                Value::Uncertain(Box::new(res), *p)
            }
        }
        (Value::Num(x), Value::Num(y)) => Value::Num(x - y),
        (Value::Vec(v1), Value::Vec(v2)) => {
            if v1.len() != v2.len() {
                return Value::Null;
            }
            let diff: Vec<f64> = v1.iter().zip(v2.iter()).map(|(x, y)| x - y).collect();
            Value::Vec(std::sync::Arc::new(diff))
        }
        _ => Value::Null,
    }
}

fn div_values(a: &Value, b: &Value) -> Value {
    match (a, b) {
        (Value::Uncertain(v1, p1), Value::Uncertain(v2, p2)) => {
            let res = div_values(v1, v2);
            if let Value::Uncertain(inner, p3) = res {
                Value::Uncertain(inner, p1 * p2 * p3)
            } else {
                Value::Uncertain(Box::new(res), p1 * p2)
            }
        }
        (Value::Uncertain(v, p), other) => {
            let res = div_values(v, other);
            if let Value::Uncertain(inner, p2) = res {
                Value::Uncertain(inner, p * p2)
            } else {
                Value::Uncertain(Box::new(res), *p)
            }
        }
        (other, Value::Uncertain(v, p)) => {
            let res = div_values(other, v);
            if let Value::Uncertain(inner, p2) = res {
                Value::Uncertain(inner, p * p2)
            } else {
                Value::Uncertain(Box::new(res), *p)
            }
        }
        (Value::Num(x), Value::Num(y)) => {
            if *y == 0.0 {
                Value::Null
            } else {
                Value::Num(x / y)
            }
        }
        _ => Value::Null,
    }
}

fn lt_values(a: &Value, b: &Value) -> Value {
    compare_values(a, b, |x, y| x < y)
}

fn gt_values(a: &Value, b: &Value) -> Value {
    compare_values(a, b, |x, y| x > y)
}

fn le_values(a: &Value, b: &Value) -> Value {
    compare_values(a, b, |x, y| x <= y)
}

fn ge_values(a: &Value, b: &Value) -> Value {
    compare_values(a, b, |x, y| x >= y)
}

fn compare_values<F>(a: &Value, b: &Value, op: F) -> Value
where
    F: Fn(f64, f64) -> bool + Copy,
{
    match (a, b) {
        (Value::Uncertain(v1, p1), Value::Uncertain(v2, p2)) => {
            let res = compare_values(v1, v2, op);
            if let Value::Uncertain(inner, p3) = res {
                Value::Uncertain(inner, p1 * p2 * p3)
            } else {
                Value::Uncertain(Box::new(res), p1 * p2)
            }
        }
        (Value::Uncertain(v, p), other) => {
            let res = compare_values(v, other, op);
            if let Value::Uncertain(inner, p2) = res {
                Value::Uncertain(inner, p * p2)
            } else {
                Value::Uncertain(Box::new(res), *p)
            }
        }
        (other, Value::Uncertain(v, p)) => {
            let res = compare_values(other, v, op);
            if let Value::Uncertain(inner, p2) = res {
                Value::Uncertain(inner, p * p2)
            } else {
                Value::Uncertain(Box::new(res), *p)
            }
        }
        (Value::Num(x), Value::Num(y)) => Value::Bool(op(*x, *y)),
        _ => Value::Bool(false), // Only numbers comparable for now
    }
}
