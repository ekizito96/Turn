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
    pub frames: Vec<Frame>,
    pub stack: Vec<Value>,
    pub runtime: Runtime,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Process {
    pub pid: u64,
    pub frames: Vec<Frame>,
    pub stack: Vec<Value>,
    pub runtime: Runtime,
    pub mailbox: VecDeque<Value>,
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
        };

        let mut scheduler = VecDeque::new();
        scheduler.push_back(root_process);

        Self {
            scheduler,
            next_pid: 2,
        }
    }

    pub fn resume_with_result(state: VmState, tool_result: Value) -> Self {
        // Reconstruct process from VmState (lossy: assumes pid 1, empty mailbox)
        let process = Process {
            pid: 1,
            frames: state.frames,
            stack: state.stack,
            runtime: state.runtime,
            mailbox: VecDeque::new(),
        };
        
        let mut scheduler = VecDeque::new();
        scheduler.push_back(process);
        
        let mut vm = Self {
            scheduler,
            next_pid: 2,
        };
        
        if let Some(p) = vm.scheduler.front_mut() {
            p.stack.push(tool_result);
        }
        
        vm
    }

    pub fn resume_with_error(state: VmState, error_msg: String) -> Self {
        let process = Process {
            pid: 1,
            frames: state.frames,
            stack: state.stack,
            runtime: state.runtime,
            mailbox: VecDeque::new(),
        };
        
        let mut scheduler = VecDeque::new();
        scheduler.push_back(process);
        
        let mut vm = Self {
            scheduler,
            next_pid: 2,
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
                    if process.pid == 1 {
                        return VmResult::Complete(v);
                    }
                    // Child finished, dropped.
                }
                VmResult::Suspended { tool_name, arg, continuation: _ } => {
                    // Reconstruct VmState for legacy support
                    let state = VmState {
                        frames: process.frames.clone(),
                        stack: process.stack.clone(),
                        runtime: process.runtime.clone(),
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
            if steps_left == 0 {
                return VmResult::Yielded;
            }
            steps_left -= 1;

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
                    if let Value::Closure { code, env, .. } = target {
                         let new_pid = self.next_pid;
                         self.next_pid += 1;
                         
                         let new_process = Process {
                             pid: new_pid,
                             frames: vec![Frame {
                                 code,
                                 ip: 0,
                                 env,
                                 handlers: Vec::new(),
                             }],
                             stack: Vec::new(),
                             runtime: Runtime::new(),
                             mailbox: VecDeque::new(),
                         };
                         
                         self.scheduler.push_back(new_process);
                         process.stack.push(Value::Pid(new_pid));
                    } else {
                        process.stack.push(Value::Null);
                    }
                }
                Instr::Send => {
                    let msg = process.stack.pop().unwrap_or(Value::Null);
                    let pid_val = process.stack.pop().unwrap_or(Value::Null);
                    
                    if let Value::Pid(pid) = pid_val {
                        let mut found = false;
                        for p in &mut self.scheduler {
                            if p.pid == pid {
                                p.mailbox.push_back(msg.clone());
                                found = true;
                                break;
                            }
                        }
                        if pid == process.pid {
                            process.mailbox.push_back(msg);
                            found = true;
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
                Instr::Confidence => {
                    let v = process.stack.pop().unwrap_or(Value::Null);
                    match v {
                        Value::Uncertain(_, p) => process.stack.push(Value::Num(p)),
                        _ => process.stack.push(Value::Num(1.0)), // Certainty
                    }
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
                    let _ = process.runtime.remember(key, val);
                }
                Instr::Add => {
                    let b = process.stack.pop().unwrap_or(Value::Null);
                    let a = process.stack.pop().unwrap_or(Value::Null);
                    process.stack.push(add_values(&a, &b));
                }
                Instr::Mul => {
                    let b = process.stack.pop().unwrap_or(Value::Null);
                    let a = process.stack.pop().unwrap_or(Value::Null);
                    process.stack.push(mul_values(&a, &b));
                }
                Instr::Eq => {
                    let b = process.stack.pop().unwrap_or(Value::Null);
                    let a = process.stack.pop().unwrap_or(Value::Null);
                    process.stack.push(eq_values(&a, &b));
                }
                Instr::Ne => {
                    let b = process.stack.pop().unwrap_or(Value::Null);
                    let a = process.stack.pop().unwrap_or(Value::Null);
                    process.stack.push(ne_values(&a, &b));
                }
                Instr::Not => {
                    let v = process.stack.pop().unwrap_or(Value::Null);
                    process.stack.push(not_value(&v));
                }
                Instr::And => {
                    let b = process.stack.pop().unwrap_or(Value::Null);
                    let a = process.stack.pop().unwrap_or(Value::Null);
                    process.stack.push(and_values(&a, &b));
                }
                Instr::Or => {
                    let b = process.stack.pop().unwrap_or(Value::Null);
                    let a = process.stack.pop().unwrap_or(Value::Null);
                    process.stack.push(or_values(&a, &b));
                }
                Instr::Jump(target) => {
                    process.frames[frame_idx].ip = target as usize;
                }
                Instr::JumpIfFalse(target) => {
                    let v = process.stack.pop().unwrap_or(Value::Null);
                    if v.is_falsy() { process.frames[frame_idx].ip = target as usize; }
                }
                Instr::JumpIfTrue(target) => {
                    let v = process.stack.pop().unwrap_or(Value::Null);
                    if v.is_truthy() { process.frames[frame_idx].ip = target as usize; }
                }
                Instr::ContextAppend => {
                    let v = process.stack.pop().unwrap_or(Value::Null);
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
                Instr::CallTool => {
                    let arg = process.stack.pop().unwrap_or(Value::Null);
                    let tool_val = process.stack.pop().unwrap_or(Value::Null);
                    
                    match tool_val {
                        Value::Str(name) => {
                            process.frames[frame_idx].env = process.runtime.env.clone();
                            let state = VmState {
                                frames: process.frames.clone(),
                                stack: process.stack.clone(),
                                runtime: process.runtime.clone(),
                            };
                            return VmResult::Suspended {
                                tool_name: name,
                                arg,
                                continuation: state,
                            };
                        }
                        Value::Closure { code, ip, env } => {
                            let mut new_env = env.clone();
                            let mut mem_inserts = Vec::new();
                            
                            if let Value::Map(m) = arg {
                                for (k, v) in m {
                                    mem_inserts.push((k.clone(), v.clone()));
                                    new_env.insert(k, v);
                                }
                            } else if let Value::Struct(_, m) = arg.clone() {
                                for (k, v) in m {
                                    mem_inserts.push((k.clone(), v.clone()));
                                    new_env.insert(k, v);
                                }
                            } else if !arg.is_falsy() {
                                mem_inserts.push(("arg".to_string(), arg.clone()));
                                new_env.insert("arg".to_string(), arg);
                            }
                            
                            process.frames[frame_idx].env = process.runtime.env.clone();
                            process.runtime.env = new_env;
                            for (k, v) in mem_inserts {
                                process.runtime.memory.insert(k, v);
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
                        _ => Value::Null,
                    };
                    process.stack.push(res);
                }
                Instr::MakeTurn(offset) => {
                    let code = process.frames[frame_idx].code.clone();
                    let env = process.runtime.env.clone();
                    process.stack.push(Value::Closure { code, ip: offset as usize, env });
                }
                Instr::LoadModule => {
                    let p_val = process.stack.pop().unwrap_or(Value::Null);
                    let path = match p_val { Value::Str(s) => s, _ => "".to_string() };
                    process.frames[frame_idx].env = process.runtime.env.clone();
                    let state = VmState { frames: process.frames.clone(), stack: process.stack.clone(), runtime: process.runtime.clone() };
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
            (Type::Pid, Value::Pid(_)) => true,
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
