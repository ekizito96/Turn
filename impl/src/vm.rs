use crate::bytecode::Instr;
use crate::runtime::Runtime;
use crate::value::Value;
use crate::ast::Type;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::{HashMap, VecDeque};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Frame {
    pub code: Arc<Vec<Instr>>,
    pub ip: usize,
    pub env: HashMap<String, Value>,
    pub handlers: Vec<u32>, // Stack of catch block offsets relative to code start
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VmState {
    pub pid: u64,
    pub frames: Vec<Frame>,
    pub stack: Vec<Value>,
    pub runtime: Runtime,
    pub mailbox: VecDeque<Value>,
    pub scheduler: VecDeque<Process>,
    pub next_pid: u64,
    pub token_budget: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Process {
    pub pid: u64,
    pub frames: Vec<Frame>,
    pub stack: Vec<Value>,
    pub runtime: Runtime,
    pub mailbox: VecDeque<Value>,
    pub token_budget: usize,
    pub links: Vec<u64>,
    pub monitors: Vec<u64>,
}

#[derive(Debug)]
pub enum VmResult {
    Complete(Value),
    Suspended {
        tool_name: String,
        arg: Value,
        continuation: VmState,
    },
    Yielded,
}

pub struct Vm {
    pub scheduler: VecDeque<Process>,
    pub next_pid: u64,
}

impl Vm {
    pub fn new(code: &[Instr]) -> Self {
        let root_frame = Frame {
            code: Arc::new(code.to_vec()),
            ip: 0,
            env: HashMap::new(),
            handlers: Vec::new(),
        };
        
        let root_process = Process {
            pid: 1,
            frames: vec![root_frame],
            stack: Vec::new(),
            runtime: Runtime::new(),
            mailbox: VecDeque::new(),
            token_budget: 1_000_000,
            links: Vec::new(),
            monitors: Vec::new(),
        };

        let mut scheduler = VecDeque::new();
        scheduler.push_back(root_process);

        Self {
            scheduler,
            next_pid: 2,
        }
    }

    pub fn resume_with_result(state: VmState, tool_result: Value) -> Self {
        let mut process = Process {
            pid: state.pid,
            frames: state.frames,
            stack: state.stack,
            runtime: state.runtime,
            mailbox: state.mailbox,
            token_budget: state.token_budget,
            links: Vec::new(), // Reconstructed processes won't have historic links in this mocked resume yet, to be solved in Durable Heap
            monitors: Vec::new(),
        };
        
        process.stack.push(tool_result);
        
        let mut scheduler = state.scheduler;
        scheduler.push_back(process);
        
        Self {
            scheduler,
            next_pid: state.next_pid,
        }
    }

    /// Snapshots the entire VM state to a durable heap file.
    pub fn snapshot(&self, current_process: &Process, path: &str) -> std::io::Result<()> {
        let state = VmState {
            pid: current_process.pid,
            frames: current_process.frames.clone(),
            stack: current_process.stack.clone(),
            runtime: current_process.runtime.clone(),
            mailbox: current_process.mailbox.clone(),
            scheduler: self.scheduler.clone(),
            next_pid: self.next_pid,
            token_budget: current_process.token_budget,
        };
        let data = serde_json::to_string_pretty(&state)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    /// Resumes the VM from a durable heap file.
    pub fn resume_from_disk(path: &str) -> std::io::Result<Self> {
        let data = std::fs::read_to_string(path)?;
        let state: VmState = serde_json::from_str(&data)?;
        
        let process = Process {
            pid: state.pid,
            frames: state.frames,
            stack: state.stack,
            runtime: state.runtime,
            mailbox: state.mailbox,
            token_budget: state.token_budget,
            // Simple reconstruction. A true WAL would rebuild the entire process table links.
            links: Vec::new(), 
            monitors: Vec::new(),
        };
        
        let mut scheduler = state.scheduler;
        scheduler.push_back(process);
        
        Ok(Self {
            scheduler,
            next_pid: state.next_pid,
        })
    }

    pub fn resume_with_error(state: VmState, error_msg: String) -> Self {
        let process = Process {
            pid: state.pid,
            frames: state.frames,
            stack: state.stack,
            runtime: state.runtime,
            mailbox: state.mailbox,
            token_budget: state.token_budget,
            links: Vec::new(),
            monitors: Vec::new(),
        };
        
        let mut scheduler = state.scheduler;
        scheduler.push_back(process);
        
        let mut vm = Self {
            scheduler,
            next_pid: state.next_pid,
        };

        let err = Value::Str(error_msg);
        
        // Unwind stack looking for catch
        if let Some(p) = vm.scheduler.front_mut() {
             loop {
                if p.frames.is_empty() {
                    // No frames left, push error to stack (uncaught)
                    p.stack.push(err);
                    break;
                }

                let f_idx = p.frames.len() - 1;
                if let Some(h_off) = p.frames[f_idx].handlers.pop() {
                    p.frames[f_idx].ip = h_off as usize;
                    p.stack.push(err);
                    break;
                } else {
                    p.frames.pop();
                    if let Some(c) = p.frames.last() {
                        p.runtime.env = c.env.clone();
                    }
                }
            }
        }
        vm
    }

    pub fn run(&mut self) -> VmResult {
        loop {
            if self.scheduler.is_empty() {
                return VmResult::Complete(Value::Null);
            }

            let mut process = self.scheduler.pop_front().unwrap();
            let result = self.run_process(&mut process, 1000); // 1000 ops slice
            
            match result {
                VmResult::Yielded => {
                    self.scheduler.push_back(process);
                }
                VmResult::Complete(v) => {
                    // Route exit signals to linked and monitored processes
                    let reason = if let Value::Str(ref _s) = v {
                        v.clone() // Propagate the specific return value or panic string
                    } else if v != Value::Null {
                        v.clone()
                    } else {
                        Value::Str("normal".to_string())
                    };

                    // Broadcast to links (bidirectional expectation, so a link crash usually crashes the parent unless trapped, but Turn handles this as a standard mailbox priority message for now)
                    for linked_pid in &process.links {
                        if let Some(target) = self.scheduler.iter_mut().find(|p| p.pid == *linked_pid) {
                            let mut map = indexmap::IndexMap::new();
                            map.insert("type".to_string(), Value::Str("EXIT".to_string()));
                            map.insert("pid".to_string(), Value::Pid { node_id: "local".to_string(), local_pid: process.pid });
                            map.insert("reason".to_string(), reason.clone());
                            target.mailbox.push_back(Value::Map(map));
                        }
                    }

                    // Broadcast to monitors (unidirectional observation)
                    for monitor_pid in &process.monitors {
                        if let Some(target) = self.scheduler.iter_mut().find(|p| p.pid == *monitor_pid) {
                            let mut map = indexmap::IndexMap::new();
                            map.insert("type".to_string(), Value::Str("DOWN".to_string()));
                            map.insert("pid".to_string(), Value::Pid { node_id: "local".to_string(), local_pid: process.pid });
                            map.insert("reason".to_string(), reason.clone());
                            target.mailbox.push_back(Value::Map(map));
                        }
                    }

                    if process.pid == 1 {
                        return VmResult::Complete(v);
                    }
                    // Child finished, dropped.
                }
                VmResult::Suspended { tool_name, arg, continuation: _ } => {
                    // Reconstruct VmState for legacy support
                    let state = VmState {
                        pid: process.pid,
                        frames: process.frames.clone(),
                        stack: process.stack.clone(),
                        runtime: process.runtime.clone(),
                        mailbox: process.mailbox.clone(),
                        scheduler: self.scheduler.clone(),
                        next_pid: self.next_pid,
                        token_budget: process.token_budget,
                    };
                    return VmResult::Suspended {
                        tool_name,
                        arg,
                        continuation: state,
                    };
                }
            }
        }
    }

    fn run_process(&mut self, process: &mut Process, steps: usize) -> VmResult {
        let mut steps_left = steps;
        
        loop {
            if process.token_budget == 0 {
                return VmResult::Complete(Value::Str("Runtime Error: TokenExhaustionError - Process ran out of gas".to_string()));
            }
        
            if steps_left == 0 {
                return VmResult::Yielded;
            }
            steps_left -= 1;
            process.token_budget = process.token_budget.saturating_sub(1);

            if process.frames.is_empty() {
                 return VmResult::Complete(process.stack.pop().unwrap_or(Value::Null));
            }

            let frame_idx = process.frames.len() - 1;
            let frame = &mut process.frames[frame_idx];
            
            if frame.ip >= frame.code.len() {
                if process.frames.len() == 1 {
                    return VmResult::Complete(process.stack.pop().unwrap_or(Value::Null));
                } else {
                    process.frames.pop();
                    if let Some(caller) = process.frames.last() {
                        process.runtime.env = caller.env.clone();
                    }
                    process.stack.push(Value::Null);
                    continue;
                }
            }

            let instr = frame.code[frame.ip].clone();
            frame.ip += 1;

            match instr {
                Instr::Spawn => {
                    let target = process.stack.pop().unwrap_or(Value::Null);
                    if let Value::Closure { code, ip, env, .. } = target {
                         let new_pid = self.next_pid;
                         self.next_pid += 1;
                         
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
                             links: Vec::new(),
                             monitors: Vec::new(),
                         };
                         new_process.runtime.env = env;
                         
                         self.scheduler.push_back(new_process);
                         process.stack.push(Value::Pid { node_id: "local".to_string(), local_pid: new_pid });
                    } else {
                        process.stack.push(Value::Null);
                    }
                }
                Instr::SpawnRemote => {
                    let closure_val = process.stack.pop().unwrap_or(Value::Null);
                    let node_ip_val = process.stack.pop().unwrap_or(Value::Null);
                    
                    if let (Value::Str(node_id), Value::Closure { .. }) = (&node_ip_val, &closure_val) {
                        let mut success = false;
                        if let Some(sb) = &process.runtime.switchboard {
                            // By convention, a Local PID of 0 on a SpawnRemote represents the "Host System" 
                            // asking to spawn a new root process on that node.
                            if sb.send_remote(node_id, 0, closure_val.clone()).is_ok() {
                                success = true;
                                // We don't have the remote PID synchronously. For a robust actor model,
                                // the remote node would send a message back with its PID.
                                // For now, we return a generic "Spawned" boolean or a proxy PID.
                                process.stack.push(Value::Bool(true));
                            }
                        }
                        
                        if !success {
                            println!("[VM] Warning: SpawnRemote to {} failed (no switchboard or network error).", node_id);
                            process.stack.push(Value::Bool(false));
                        }
                    } else {
                        process.stack.push(Value::Null);
                    }
                }
                Instr::Send => {
                    let msg = process.stack.pop().unwrap_or(Value::Null);
                    let pid_val = process.stack.pop().unwrap_or(Value::Null);
                    
                    if let Value::Pid { node_id, local_pid } = pid_val {
                        let mut found = false;
                        if node_id != "local" {
                            // Remote TCP/gRPC Proxy via Host Switchboard
                            if let Some(sb) = &process.runtime.switchboard {
                                if sb.send_remote(&node_id, local_pid, msg.clone()).is_ok() {
                                    found = true;
                                }
                            } else {
                                println!("[VM] Warning: Remote send attempted to {}, but no NetworkSwitchboard attached.", node_id);
                            }
                        } else {
                            // Local Memory Proxy
                            for p in &mut self.scheduler {
                                if p.pid == local_pid {
                                    p.mailbox.push_back(msg.clone());
                                    found = true;
                                    break;
                                }
                            }
                            if local_pid == process.pid {
                                process.mailbox.push_back(msg);
                                found = true;
                            }
                        }
                        process.stack.push(Value::Bool(found));
                    } else {
                         process.stack.push(Value::Bool(false));
                    }
                }
                Instr::Receive => {
                    if let Some(msg) = process.mailbox.pop_front() {
                        process.stack.push(msg);
                    } else {
                        // Yield and retry same instruction later
                        process.frames[frame_idx].ip -= 1;
                        return VmResult::Yielded; 
                    }
                }
                Instr::Link => {
                    let pid_val = process.stack.pop().unwrap_or(Value::Null);
                    if let Value::Pid { node_id, local_pid } = pid_val {
                        // TODO: Distributed links
                        if node_id == "local" && !process.links.contains(&local_pid) {
                            process.links.push(local_pid);
                            // Bidirectional link: add ourselves to the target's links
                            for target in &mut self.scheduler {
                                if target.pid == local_pid {
                                    if !target.links.contains(&process.pid) {
                                        target.links.push(process.pid);
                                    }
                                    break;
                                }
                            }
                        }
                        process.stack.push(Value::Bool(true));
                    } else {
                        process.stack.push(Value::Bool(false));
                    }
                }
                Instr::Monitor => {
                    let pid_val = process.stack.pop().unwrap_or(Value::Null);
                    if let Value::Pid { node_id, local_pid } = pid_val {
                        // Unidirectional monitor: Add ourselves to target's monitors array
                        let mut found = false;
                        if node_id == "local" {
                            for target in &mut self.scheduler {
                                if target.pid == local_pid {
                                    if !target.monitors.contains(&process.pid) {
                                        target.monitors.push(process.pid);
                                    }
                                    found = true;
                                    break;
                                }
                            }
                        }
                        process.stack.push(Value::Bool(found));
                    } else {
                        process.stack.push(Value::Bool(false));
                    }
                }
                Instr::Confidence => {
                    let v = process.stack.pop().unwrap_or(Value::Null);
                    match v {
                        Value::Uncertain(_, p) => process.stack.push(Value::Num(p)),
                        _ => process.stack.push(Value::Num(1.0)), // Certainty
                    }
                }
                Instr::Suspend => {
                    process.frames[frame_idx].env = process.runtime.env.clone();
                    let state = VmState {
                        pid: process.pid,
                        frames: process.frames.clone(),
                        stack: process.stack.clone(),
                        runtime: process.runtime.clone(),
                        mailbox: process.mailbox.clone(),
                        scheduler: self.scheduler.clone(),
                        next_pid: self.next_pid,
                        token_budget: process.token_budget,
                    };
                    
                    // Orthogonal Persistence: Flush active memory to local WAL
                    let _ = self.snapshot(&process, ".turn_heap.json");
                    
                    return VmResult::Suspended {
                        tool_name: "sys_suspend".to_string(),
                        arg: Value::Null,
                        continuation: state,
                    };
                }
                Instr::Infer(ty, tool_count) => {
                    let mut tools = Vec::new();
                    for _ in 0..tool_count {
                        tools.push(process.stack.pop().unwrap_or(Value::Null));
                    }
                    tools.reverse(); // stack pops in reverse order

                    let prompt_val = process.stack.pop().unwrap_or(Value::Null);
                    // Resolve named struct placeholders (Struct("Name", {}))
                    // against runtime-registered struct definitions before prompting LLM.
                    let resolved_ty = match ty {
                        Type::Struct(name, fields) if fields.is_empty() => {
                            if let Some(known_fields) = process.runtime.structs.get(&name) {
                                Type::Struct(name, known_fields.clone())
                            } else {
                                Type::Struct(name, fields)
                            }
                        }
                        other => other,
                    };
                    let ty_str = serde_json::to_string(&resolved_ty).unwrap_or_else(|_| "{}".to_string());
                    
                    // Semantic Auto-Recall (Phase 3)
                    // Take the user's prompt string, generate a vector representation behind the scenes,
                    // query the HNSW Semantic Graph for the Top 3 most semantically similar memories,
                    // and prepend them into the Token Budget context payload automatically.
                    let mut auto_context = Vec::new();
                    if let Value::Str(prompt_str) = &prompt_val {
                        if let Some(query_emb) = crate::llm_tools::get_embedding(prompt_str) {
                            let semantic_matches = process.runtime.memory.search(&query_emb, 3);
                            for memory_val in semantic_matches {
                                auto_context.push(memory_val);
                            }
                            // Bill tokens for the vector similarity operation
                            if process.token_budget >= 50 {
                                process.token_budget -= 50;
                            }
                        }
                    }
                    
                    // Combine Auto-Recall + Explicit Context stack
                    let mut combined_context = auto_context;
                    combined_context.extend(process.runtime.context.to_values());
                    
                    let mut map = IndexMap::new();
                    map.insert("prompt".to_string(), prompt_val);
                    map.insert("schema".to_string(), Value::Str(ty_str));
                    map.insert("context".to_string(), Value::List(combined_context));
                    if tool_count > 0 {
                        map.insert("tools".to_string(), Value::List(tools));
                    }
                    
                    process.frames[frame_idx].env = process.runtime.env.clone();
                    let state = VmState {
                        pid: process.pid,
                        frames: process.frames.clone(),
                        stack: process.stack.clone(),
                        runtime: process.runtime.clone(),
                        mailbox: process.mailbox.clone(),
                        scheduler: self.scheduler.clone(),
                        next_pid: self.next_pid,
                        token_budget: process.token_budget,
                    };
                    return VmResult::Suspended {
                        tool_name: "llm_infer".to_string(),
                        arg: Value::Map(map),
                        continuation: state,
                    };
                }
                Instr::DefineStruct(name, fields) => {
                    process.runtime.register_struct(name, fields);
                }
                Instr::PushNull => process.stack.push(Value::Null),
                Instr::PushTrue => process.stack.push(Value::Bool(true)),
                Instr::PushFalse => process.stack.push(Value::Bool(false)),
                Instr::PushNum(n) => process.stack.push(Value::Num(n)),
                Instr::PushStr(s) => process.stack.push(Value::Str(s)),
                Instr::Pop => { process.stack.pop(); },
                Instr::Load(name) => {
                    match process.runtime.get_env(&name) {
                        Some(v) => process.stack.push(v),
                        None => process.stack.push(Value::Null),
                    }
                }
                Instr::Store(name) => {
                    let val = process.stack.pop().unwrap_or(Value::Null);
                    process.runtime.push_env(name, val);
                }
                Instr::Recall => {
                    let key = process.stack.pop().unwrap_or(Value::Null);
                    let val = process.runtime.recall(&key);
                    process.stack.push(val);
                }
                Instr::Remember => {
                    let val = process.stack.pop().unwrap_or(Value::Null);
                    let key = process.stack.pop().unwrap_or(Value::Null);
                    
                    // Auto-Embedding Logic for Semantic RAM
                    // If the user did not provide an explicit embedding inside a Struct/Map,
                    // the Virtual Machine generates one implicitly via Azure OpenAI
                    let (mut id, mut emb) = match &key {
                        Value::Map(m) | Value::Struct(_, m) => {
                            let k_id = m.get("id").and_then(|v| crate::runtime::value_to_key(v).ok()).unwrap_or_else(|| "unnamed".to_string());
                            let k_emb = match m.get("embedding") {
                                Some(Value::Vec(v)) => Some(v.clone()),
                                _ => None,
                            };
                            (k_id, k_emb)
                        }
                        _ => (crate::runtime::value_to_key(&key).unwrap_or_default(), None)
                    };
                    
                    if emb.is_none() && !id.is_empty() {
                         // Generate implicit vector coordinates representing the semantic memory
                         emb = crate::llm_tools::get_embedding(&id);
                         // Bill the process for the background LLM operation
                         if process.token_budget >= 50 {
                             process.token_budget -= 50;
                         }
                    }
                    
                    process.runtime.memory.insert(id, emb, val);
                }
                Instr::Add => {
                    let b = process.stack.pop().unwrap_or(Value::Null);
                    let a = process.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) { return VmResult::Complete(Value::Str("PrivilegeViolation: Opaque capabilities cannot be observed".to_string())); }
                    process.stack.push(add_values(&a, &b));
                }
                Instr::Sub => {
                    let b = process.stack.pop().unwrap_or(Value::Null);
                    let a = process.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) { return VmResult::Complete(Value::Str("PrivilegeViolation: Opaque capabilities cannot be observed".to_string())); }
                    process.stack.push(sub_values(&a, &b));
                }
                Instr::Mul => {
                    let b = process.stack.pop().unwrap_or(Value::Null);
                    let a = process.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) { return VmResult::Complete(Value::Str("PrivilegeViolation: Opaque capabilities cannot be observed".to_string())); }
                    process.stack.push(mul_values(&a, &b));
                }
                Instr::Div => {
                    let b = process.stack.pop().unwrap_or(Value::Null);
                    let a = process.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) { return VmResult::Complete(Value::Str("PrivilegeViolation: Opaque capabilities cannot be observed".to_string())); }
                    process.stack.push(div_values(&a, &b));
                }
                Instr::Eq => {
                    let b = process.stack.pop().unwrap_or(Value::Null);
                    let a = process.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) { return VmResult::Complete(Value::Str("PrivilegeViolation: Opaque capabilities cannot be observed".to_string())); }
                    process.stack.push(eq_values(&a, &b));
                }
                Instr::Ne => {
                    let b = process.stack.pop().unwrap_or(Value::Null);
                    let a = process.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) { return VmResult::Complete(Value::Str("PrivilegeViolation: Opaque capabilities cannot be observed".to_string())); }
                    process.stack.push(ne_values(&a, &b));
                }
                Instr::Lt => {
                    let b = process.stack.pop().unwrap_or(Value::Null);
                    let a = process.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) { return VmResult::Complete(Value::Str("PrivilegeViolation: Opaque capabilities cannot be observed".to_string())); }
                    process.stack.push(lt_values(&a, &b));
                }
                Instr::Gt => {
                    let b = process.stack.pop().unwrap_or(Value::Null);
                    let a = process.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) { return VmResult::Complete(Value::Str("PrivilegeViolation: Opaque capabilities cannot be observed".to_string())); }
                    process.stack.push(gt_values(&a, &b));
                }
                Instr::Le => {
                    let b = process.stack.pop().unwrap_or(Value::Null);
                    let a = process.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) { return VmResult::Complete(Value::Str("PrivilegeViolation: Opaque capabilities cannot be observed".to_string())); }
                    process.stack.push(le_values(&a, &b));
                }
                Instr::Ge => {
                    let b = process.stack.pop().unwrap_or(Value::Null);
                    let a = process.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) { return VmResult::Complete(Value::Str("PrivilegeViolation: Opaque capabilities cannot be observed".to_string())); }
                    process.stack.push(ge_values(&a, &b));
                }
                Instr::Not => {
                    let v = process.stack.pop().unwrap_or(Value::Null);
                    if matches!(v, Value::Cap(_)) { return VmResult::Complete(Value::Str("PrivilegeViolation: Opaque capabilities cannot be observed".to_string())); }
                    process.stack.push(not_value(&v));
                }
                Instr::And => {
                    let b = process.stack.pop().unwrap_or(Value::Null);
                    let a = process.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) { return VmResult::Complete(Value::Str("PrivilegeViolation: Opaque capabilities cannot be observed".to_string())); }
                    process.stack.push(and_values(&a, &b));
                }
                Instr::Or => {
                    let b = process.stack.pop().unwrap_or(Value::Null);
                    let a = process.stack.pop().unwrap_or(Value::Null);
                    if matches!(a, Value::Cap(_)) || matches!(b, Value::Cap(_)) { return VmResult::Complete(Value::Str("PrivilegeViolation: Opaque capabilities cannot be observed".to_string())); }
                    process.stack.push(or_values(&a, &b));
                }
                Instr::Jump(target) => {
                    process.frames[frame_idx].ip = target as usize;
                }
                Instr::JumpIfFalse(target) => {
                    let v = process.stack.pop().unwrap_or(Value::Null);
                    if matches!(v, Value::Cap(_)) { return VmResult::Complete(Value::Str("PrivilegeViolation: Opaque capabilities cannot be branched upon".to_string())); }
                    if v.is_falsy() { process.frames[frame_idx].ip = target as usize; }
                }
                Instr::JumpIfTrue(target) => {
                    let v = process.stack.pop().unwrap_or(Value::Null);
                    if matches!(v, Value::Cap(_)) { return VmResult::Complete(Value::Str("PrivilegeViolation: Opaque capabilities cannot be branched upon".to_string())); }
                    if v.is_truthy() { process.frames[frame_idx].ip = target as usize; }
                }
                Instr::ContextAppend => {
                    let v = process.stack.pop().unwrap_or(Value::Null);
                    if matches!(v, Value::Cap(_)) { return VmResult::Complete(Value::Str("PrivilegeViolation: Opaque capabilities cannot be appended to Agent context".to_string())); }
                    let _ = process.runtime.append_context(v);
                }
                Instr::EnterTurn(after_addr) => {
                    let current_ip = process.frames[frame_idx].ip;
                    process.frames[frame_idx].ip = after_addr as usize;
                    let code = process.frames[frame_idx].code.clone();
                    let env = process.runtime.env.clone();
                    process.frames.push(Frame {
                        code,
                        ip: current_ip,
                        env,
                        handlers: Vec::new(),
                    });
                }
                Instr::PushHandler(offset) => {
                    process.frames[frame_idx].handlers.push(offset);
                }
                Instr::PopHandler => {
                    process.frames[frame_idx].handlers.pop();
                }
                Instr::Throw => {
                    let err = process.stack.pop().unwrap_or(Value::Null);
                    loop {
                        if process.frames.is_empty() {
                            return VmResult::Complete(err);
                        }
                        let f_idx = process.frames.len() - 1;
                        if let Some(h_off) = process.frames[f_idx].handlers.pop() {
                            process.frames[f_idx].ip = h_off as usize;
                            process.stack.push(err);
                            break;
                        } else {
                            process.frames.pop();
                            if let Some(caller) = process.frames.last() {
                                process.runtime.env = caller.env.clone();
                            }
                        }
                    }
                }
                Instr::CallMethod(name) => {
                    let arg = process.stack.pop().unwrap_or(Value::Null);
                    let target = process.stack.pop().unwrap_or(Value::Null);
                    
                    let (tool_val, final_arg) = if let Some(func) = match &target {
                        Value::Map(m) | Value::Struct(_, m) => m.get(&name).cloned(),
                        _ => None
                    } {
                        (func, arg)
                    } else if let Some(func) = process.runtime.get_env(&name) {
                        let final_arg = if arg.is_falsy() { target } else { arg };
                        (func, final_arg)
                    } else {
                        (Value::Null, Value::Null)
                    };

                    match tool_val {
                        Value::Str(name) => {
                            process.frames[frame_idx].env = process.runtime.env.clone();
                            let state = VmState {
                                pid: process.pid,
                                frames: process.frames.clone(),
                                stack: process.stack.clone(),
                                runtime: process.runtime.clone(),
                                mailbox: process.mailbox.clone(),
                                scheduler: self.scheduler.clone(),
                                next_pid: self.next_pid,
                                token_budget: process.token_budget,
                            };
                            return VmResult::Suspended {
                                tool_name: name,
                                arg: final_arg,
                                continuation: state,
                            };
                        }
                        Value::Closure { is_tool: _, code, ip, env, params } => {
                            let mut new_env = env.clone();
                            let mut mem_inserts = Vec::new();

                            if params.len() == 1 {
                                let name = &params[0].0;
                                match final_arg {
                                    Value::Map(m) => {
                                        if m.contains_key(name) {
                                            for (k, v) in m {
                                                mem_inserts.push((k.clone(), v.clone()));
                                                new_env.insert(k, v);
                                            }
                                        } else {
                                            let wrapped = Value::Map(m);
                                            mem_inserts.push((name.clone(), wrapped.clone()));
                                            new_env.insert(name.clone(), wrapped);
                                        }
                                    }
                                    Value::Struct(struct_name, m) => {
                                        if m.contains_key(name) {
                                            for (k, v) in m {
                                                mem_inserts.push((k.clone(), v.clone()));
                                                new_env.insert(k, v);
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
                                for (k, v) in m {
                                    mem_inserts.push((k.clone(), v.clone()));
                                    new_env.insert(k, v);
                                }
                            } else if let Value::Struct(_, m) = final_arg.clone() {
                                for (k, v) in m {
                                    mem_inserts.push((k.clone(), v.clone()));
                                    new_env.insert(k, v);
                                }
                            } else if let Value::List(items) = final_arg {
                                for (i, item) in items.into_iter().enumerate() {
                                    if i < params.len() {
                                        mem_inserts.push((params[i].0.clone(), item.clone()));
                                        new_env.insert(params[i].0.clone(), item);
                                    }
                                }
                            } else if !final_arg.is_falsy() {
                                mem_inserts.push(("arg".to_string(), final_arg.clone()));
                                new_env.insert("arg".to_string(), final_arg);
                            }
                            
                            process.frames[frame_idx].env = process.runtime.env.clone();
                            process.runtime.env = new_env;
                            for (k, v) in mem_inserts {
                                process.runtime.memory.insert(k, None, v);
                            }
                            
                            process.frames.push(Frame {
                                code,
                                ip,
                                env: process.runtime.env.clone(),
                                handlers: Vec::new(),
                            });
                        }
                        _ => process.stack.push(Value::Null),
                    }
                }
                Instr::CallTool => {
                    let arg = process.stack.pop().unwrap_or(Value::Null);
                    let tool_val = process.stack.pop().unwrap_or(Value::Null);
                    
                    match tool_val {
                        Value::Str(name) => {
                            process.frames[frame_idx].env = process.runtime.env.clone();
                            let state = VmState {
                                pid: process.pid,
                                frames: process.frames.clone(),
                                stack: process.stack.clone(),
                                runtime: process.runtime.clone(),
                                mailbox: process.mailbox.clone(),
                                scheduler: self.scheduler.clone(),
                                next_pid: self.next_pid,
                                token_budget: process.token_budget,
                            };
                            return VmResult::Suspended {
                                tool_name: name,
                                arg,
                                continuation: state,
                            };
                        }
                        Value::Closure { is_tool: _, code, ip, env, params } => {
                            let mut new_env = env.clone();
                            let mut mem_inserts = Vec::new();

                            if params.len() == 1 {
                                let name = &params[0].0;
                                match arg {
                                    Value::Map(m) => {
                                        if m.contains_key(name) {
                                            for (k, v) in m {
                                                mem_inserts.push((k.clone(), v.clone()));
                                                new_env.insert(k, v);
                                            }
                                        } else {
                                            let wrapped = Value::Map(m);
                                            mem_inserts.push((name.clone(), wrapped.clone()));
                                            new_env.insert(name.clone(), wrapped);
                                        }
                                    }
                                    Value::Struct(struct_name, m) => {
                                        if m.contains_key(name) {
                                            for (k, v) in m {
                                                mem_inserts.push((k.clone(), v.clone()));
                                                new_env.insert(k, v);
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
                                for (k, v) in m {
                                    mem_inserts.push((k.clone(), v.clone()));
                                    new_env.insert(k, v);
                                }
                            } else if let Value::Struct(_, m) = arg.clone() {
                                for (k, v) in m {
                                    mem_inserts.push((k.clone(), v.clone()));
                                    new_env.insert(k, v);
                                }
                            } else if let Value::List(items) = arg {
                                for (i, item) in items.into_iter().enumerate() {
                                    if i < params.len() {
                                        mem_inserts.push((params[i].0.clone(), item.clone()));
                                        new_env.insert(params[i].0.clone(), item);
                                    }
                                }
                            } else if !arg.is_falsy() {
                                mem_inserts.push(("arg".to_string(), arg.clone()));
                                new_env.insert("arg".to_string(), arg);
                            }
                            
                            process.frames[frame_idx].env = process.runtime.env.clone();
                            process.runtime.env = new_env;
                            for (k, v) in mem_inserts {
                                process.runtime.memory.insert(k, None, v);
                            }
                            
                            process.frames.push(Frame {
                                code,
                                ip,
                                env: process.runtime.env.clone(),
                                handlers: Vec::new(),
                            });
                        }
                        _ => process.stack.push(Value::Null),
                    }
                }
                Instr::Return => {
                    let ret_val = process.stack.pop().unwrap_or(Value::Null);
                    if process.frames.len() > 1 {
                        process.frames.pop();
                        if let Some(caller) = process.frames.last() {
                            process.runtime.env = caller.env.clone();
                        }
                        process.stack.push(ret_val);
                    } else {
                        return VmResult::Complete(ret_val);
                    }
                }
                Instr::MakeList(count) => {
                    let mut items = Vec::new();
                    for _ in 0..count { items.push(process.stack.pop().unwrap_or(Value::Null)); }
                    items.reverse();
                    process.stack.push(Value::List(items));
                }
                Instr::MakeMap(count) => {
                    let mut map = IndexMap::new();
                    for _ in 0..count {
                        let val = process.stack.pop().unwrap_or(Value::Null);
                        let k_val = process.stack.pop().unwrap_or(Value::Null);
                        let k = match k_val { Value::Str(s) => s, _ => k_val.to_string() };
                        map.insert(k, val);
                    }
                    process.stack.push(Value::Map(map));
                }
                Instr::MakeStruct(name, count) => {
                    let mut map = IndexMap::new();
                    for _ in 0..count {
                        let val = process.stack.pop().unwrap_or(Value::Null);
                        let k_val = process.stack.pop().unwrap_or(Value::Null);
                        let k = match k_val { Value::Str(s) => s, _ => k_val.to_string() };
                        map.insert(k, val);
                    }
                    process.stack.push(Value::Struct(name, map));
                }
                Instr::MakeVec(count) => {
                    let mut items = Vec::new();
                    for _ in 0..count {
                        let v = process.stack.pop().unwrap_or(Value::Null);
                        if let Value::Num(n) = v {
                            items.push(n);
                        } else {
                            // Runtime error: Vector elements must be numbers
                            items.push(0.0); // Fallback
                        }
                    }
                    items.reverse();
                    process.stack.push(Value::Vec(items));
                }
                Instr::Similarity => {
                    let b = process.stack.pop().unwrap_or(Value::Null);
                    let a = process.stack.pop().unwrap_or(Value::Null);
                    if let (Value::Vec(v1), Value::Vec(v2)) = (a, b) {
                        process.stack.push(Value::Num(cosine_similarity(&v1, &v2)));
                    } else {
                        process.stack.push(Value::Null);
                    }
                }
                Instr::Index => {
                    let idx = process.stack.pop().unwrap_or(Value::Null);
                    let tgt = process.stack.pop().unwrap_or(Value::Null);
                    let res = match tgt {
                        Value::List(l) => if let Value::Num(n) = idx { l.get(n as usize).cloned().unwrap_or(Value::Null) } else { Value::Null },
                        Value::Map(m) | Value::Struct(_, m) => {
                            let k = match idx { Value::Str(s) => s, Value::Num(n) => n.to_string(), _ => "".to_string() };
                            m.get(&k).cloned().unwrap_or(Value::Null)
                        },
                        Value::Uncertain(inner, _) => {
                             // Auto-unwrap uncertain for property access
                             match inner.as_ref() {
                                 Value::Map(m) | Value::Struct(_, m) => {
                                     let k = match idx { Value::Str(s) => s, Value::Num(n) => n.to_string(), _ => "".to_string() };
                                     m.get(&k).cloned().unwrap_or(Value::Null)
                                 },
                                 _ => Value::Null
                             }
                        },
                        _ => Value::Null,
                    };
                    process.stack.push(res);
                }
                Instr::MakeTurn(offset, is_tool, params) => {
                    let code = process.frames[frame_idx].code.clone();
                    let env = process.runtime.env.clone();
                    process.stack.push(Value::Closure { is_tool, code, ip: offset as usize, env, params: params.clone() });
                }
                Instr::LoadModule => {
                    let p_val = process.stack.pop().unwrap_or(Value::Null);
                    let path = match p_val { Value::Str(s) => s, _ => "".to_string() };
                    process.frames[frame_idx].env = process.runtime.env.clone();
                    let state = VmState {
                        pid: process.pid,
                        frames: process.frames.clone(),
                        stack: process.stack.clone(),
                        runtime: process.runtime.clone(),
                        mailbox: process.mailbox.clone(),
                        scheduler: self.scheduler.clone(),
                        next_pid: self.next_pid,
                        token_budget: process.token_budget,
                    };
                    return VmResult::Suspended { tool_name: "sys_import".to_string(), arg: Value::Str(path), continuation: state };
                }
                Instr::CheckType(ref ty) => {
                    let val = process.stack.last().unwrap_or(&Value::Null);
                    if !self.check_value_type(ty, val) {
                        let err = Value::Str(format!("Runtime Type Error: Expected {:?}, got {:?}", ty, val));
                         // Unwind
                         loop {
                             if process.frames.is_empty() { return VmResult::Complete(err); }
                             let f_idx = process.frames.len() - 1;
                             if let Some(off) = process.frames[f_idx].handlers.pop() {
                                 process.frames[f_idx].ip = off as usize;
                                 process.stack.push(err);
                                 break;
                             } else {
                                 process.frames.pop();
                                 if let Some(c) = process.frames.last() { process.runtime.env = c.env.clone(); }
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
                if **inner == Type::Any { return true; }
                for item in items {
                    if !self.check_value_type(inner, item) { return false; }
                }
                true
            },
            (Type::Map(inner), Value::Map(map)) => {
                if **inner == Type::Any { return true; }
                for (_, val) in map {
                    if !self.check_value_type(inner, val) { return false; }
                }
                true
            },
            (Type::Struct(name, fields), Value::Struct(val_name, val_fields)) => {
                if name != val_name { return false; }
                for (field_name, field_ty) in fields {
                    if let Some(val) = val_fields.get(field_name) {
                        if !self.check_value_type(field_ty, val) { return false; }
                    } else {
                        return false;
                    }
                }
                true
            },
            (Type::Function(_arg_ty, _ret_ty), Value::Closure { .. }) => true,
            (Type::Pid, Value::Pid { .. }) => true,
            (Type::Any, _) => true,
            (Type::Void, Value::Null) => true,
            _ => false,
        }
    }

    // Helper to push to CURRENT running process stack (root or first available)
    // Used by tests mainly.
    pub fn push(&mut self, v: Value) {
        if let Some(p) = self.scheduler.front_mut() {
            p.stack.push(v);
        }
    }

    pub fn pop(&mut self) -> Option<Value> {
        self.scheduler.front_mut().and_then(|p| p.stack.pop())
    }

    pub fn peek(&self) -> Option<&Value> {
        self.scheduler.front().and_then(|p| p.stack.last())
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
        },
        (Value::Uncertain(v, p), other) => {
            let res = add_values(v, other);
            if let Value::Uncertain(inner, p2) = res {
                Value::Uncertain(inner, p * p2)
            } else {
                Value::Uncertain(Box::new(res), *p)
            }
        },
        (other, Value::Uncertain(v, p)) => {
            let res = add_values(other, v);
            if let Value::Uncertain(inner, p2) = res {
                Value::Uncertain(inner, p * p2)
            } else {
                Value::Uncertain(Box::new(res), *p)
            }
        },
        (Value::Num(x), Value::Num(y)) => Value::Num(x + y),
        (Value::Vec(v1), Value::Vec(v2)) => {
            if v1.len() != v2.len() { return Value::Null; }
            let sum: Vec<f64> = v1.iter().zip(v2.iter()).map(|(x, y)| x + y).collect();
            Value::Vec(sum)
        },
        (Value::List(l1), Value::List(l2)) => {
            let mut new_list = l1.clone();
            new_list.extend(l2.clone());
            Value::List(new_list)
        },
        (Value::Map(m1), Value::Map(m2)) => {
            let mut new_map = m1.clone();
            for (k, v) in m2 {
                new_map.insert(k.clone(), v.clone());
            }
            Value::Map(new_map)
        },
        (Value::Struct(name1, m1), Value::Struct(name2, m2)) if name1 == name2 => {
             let mut new_map = m1.clone();
             for (k, v) in m2 {
                 new_map.insert(k.clone(), v.clone());
             }
             Value::Struct(name1.clone(), new_map)
        },
        _ => Value::Str(format!("{}{}", a, b)),
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
        },
        (Value::Uncertain(v, p), other) => {
            let res = mul_values(v, other);
            if let Value::Uncertain(inner, p2) = res {
                Value::Uncertain(inner, p * p2)
            } else {
                Value::Uncertain(Box::new(res), *p)
            }
        },
        (other, Value::Uncertain(v, p)) => {
            let res = mul_values(other, v);
            if let Value::Uncertain(inner, p2) = res {
                Value::Uncertain(inner, p * p2)
            } else {
                Value::Uncertain(Box::new(res), *p)
            }
        },
        (Value::Num(x), Value::Num(y)) => Value::Num(x * y),
        (Value::Vec(v), Value::Num(x)) | (Value::Num(x), Value::Vec(v)) => {
            let res: Vec<f64> = v.iter().map(|n| n * x).collect();
            Value::Vec(res)
        },
        (Value::Vec(v1), Value::Vec(v2)) => {
            if v1.len() != v2.len() { return Value::Null; }
            let dot: f64 = v1.iter().zip(v2.iter()).map(|(x, y)| x * y).sum();
            Value::Num(dot)
        },
        _ => Value::Null,
    }
}

fn cosine_similarity(v1: &[f64], v2: &[f64]) -> f64 {
    if v1.len() != v2.len() { return 0.0; }
    let dot: f64 = v1.iter().zip(v2.iter()).map(|(x, y)| x * y).sum();
    let mag1: f64 = v1.iter().map(|x| x * x).sum::<f64>().sqrt();
    let mag2: f64 = v2.iter().map(|x| x * x).sum::<f64>().sqrt();
    if mag1 == 0.0 || mag2 == 0.0 { return 0.0; }
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
        },
        (Value::Uncertain(v, p), other) => {
            let res = eq_values(v, other);
            if let Value::Uncertain(inner, p2) = res {
                Value::Uncertain(inner, p * p2)
            } else {
                Value::Uncertain(Box::new(res), *p)
            }
        },
        (other, Value::Uncertain(v, p)) => {
            let res = eq_values(other, v);
            if let Value::Uncertain(inner, p2) = res {
                Value::Uncertain(inner, p * p2)
            } else {
                Value::Uncertain(Box::new(res), *p)
            }
        },
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
        },
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
        },
        (Value::Uncertain(v, p), other) => {
            let res = and_values(v, other);
            if let Value::Uncertain(inner, p2) = res {
                Value::Uncertain(inner, p * p2)
            } else {
                Value::Uncertain(Box::new(res), *p)
            }
        },
        (other, Value::Uncertain(v, p)) => {
            let res = and_values(other, v);
            if let Value::Uncertain(inner, p2) = res {
                Value::Uncertain(inner, p * p2)
            } else {
                Value::Uncertain(Box::new(res), *p)
            }
        },
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
        },
        (Value::Uncertain(v, p), other) => {
            let res = or_values(v, other);
            Value::Uncertain(Box::new(res), *p)
        },
        (other, Value::Uncertain(v, p)) => {
            let res = or_values(other, v);
            Value::Uncertain(Box::new(res), *p)
        },
        _ => Value::Bool(a.is_truthy() || b.is_truthy()),
    }
}

fn sub_values(a: &Value, b: &Value) -> Value {
    match (a, b) {
        (Value::Uncertain(v1, p1), Value::Uncertain(v2, p2)) => {
            let res = sub_values(v1, v2);
            if let Value::Uncertain(inner, p3) = res { Value::Uncertain(inner, p1 * p2 * p3) } else { Value::Uncertain(Box::new(res), p1 * p2) }
        },
        (Value::Uncertain(v, p), other) => {
            let res = sub_values(v, other);
            if let Value::Uncertain(inner, p2) = res { Value::Uncertain(inner, p * p2) } else { Value::Uncertain(Box::new(res), *p) }
        },
        (other, Value::Uncertain(v, p)) => {
            let res = sub_values(other, v);
            if let Value::Uncertain(inner, p2) = res { Value::Uncertain(inner, p * p2) } else { Value::Uncertain(Box::new(res), *p) }
        },
        (Value::Num(x), Value::Num(y)) => Value::Num(x - y),
        (Value::Vec(v1), Value::Vec(v2)) => {
            if v1.len() != v2.len() { return Value::Null; }
            let diff: Vec<f64> = v1.iter().zip(v2.iter()).map(|(x, y)| x - y).collect();
            Value::Vec(diff)
        },
        _ => Value::Null,
    }
}

fn div_values(a: &Value, b: &Value) -> Value {
    match (a, b) {
        (Value::Uncertain(v1, p1), Value::Uncertain(v2, p2)) => {
            let res = div_values(v1, v2);
            if let Value::Uncertain(inner, p3) = res { Value::Uncertain(inner, p1 * p2 * p3) } else { Value::Uncertain(Box::new(res), p1 * p2) }
        },
        (Value::Uncertain(v, p), other) => {
            let res = div_values(v, other);
            if let Value::Uncertain(inner, p2) = res { Value::Uncertain(inner, p * p2) } else { Value::Uncertain(Box::new(res), *p) }
        },
        (other, Value::Uncertain(v, p)) => {
            let res = div_values(other, v);
            if let Value::Uncertain(inner, p2) = res { Value::Uncertain(inner, p * p2) } else { Value::Uncertain(Box::new(res), *p) }
        },
        (Value::Num(x), Value::Num(y)) => {
            if *y == 0.0 { Value::Null } else { Value::Num(x / y) }
        },
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
where F: Fn(f64, f64) -> bool + Copy {
    match (a, b) {
        (Value::Uncertain(v1, p1), Value::Uncertain(v2, p2)) => {
            let res = compare_values(v1, v2, op);
            if let Value::Uncertain(inner, p3) = res { Value::Uncertain(inner, p1 * p2 * p3) } else { Value::Uncertain(Box::new(res), p1 * p2) }
        },
        (Value::Uncertain(v, p), other) => {
            let res = compare_values(v, other, op);
            if let Value::Uncertain(inner, p2) = res { Value::Uncertain(inner, p * p2) } else { Value::Uncertain(Box::new(res), *p) }
        },
        (other, Value::Uncertain(v, p)) => {
            let res = compare_values(other, v, op);
            if let Value::Uncertain(inner, p2) = res { Value::Uncertain(inner, p * p2) } else { Value::Uncertain(Box::new(res), *p) }
        },
        (Value::Num(x), Value::Num(y)) => Value::Bool(op(*x, *y)),
        _ => Value::Bool(false), // Only numbers comparable for now
    }
}
