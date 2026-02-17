use crate::bytecode::Instr;
use crate::runtime::{Runtime, RuntimeError};
use crate::value::Value;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone)]
pub struct Frame {
    pub code: Arc<Vec<Instr>>,
    pub ip: usize,
    pub env: HashMap<String, Value>,
    pub handlers: Vec<u32>, // Stack of catch block offsets relative to code start
}

#[derive(Serialize, Deserialize, Clone)]
pub struct VmState {
    pub frames: Vec<Frame>,
    pub stack: Vec<Value>,
    pub runtime: Runtime,
}

pub enum VmResult {
    Complete(Value),
    Suspended {
        tool_name: String,
        arg: Value,
        continuation: VmState,
    },
}

pub struct Vm {
    frames: Vec<Frame>,
    stack: Vec<Value>,
    runtime: Runtime,
}

impl Vm {
    pub fn new(code: &[Instr]) -> Self {
        let root_frame = Frame {
            code: Arc::new(code.to_vec()),
            ip: 0,
            env: HashMap::new(),
            handlers: Vec::new(),
        };
        Self {
            frames: vec![root_frame],
            stack: Vec::new(),
            runtime: Runtime::new(),
        }
    }

    pub fn resume_with_result(
        state: VmState,
        _code: &[Instr], // Ignored
        tool_result: Value,
    ) -> Self {
        let mut vm = Self {
            frames: state.frames,
            stack: state.stack,
            runtime: state.runtime,
        };
        vm.push(tool_result);
        vm
    }

    pub fn run(&mut self) -> VmResult {
        loop {
            // Check if done (no frames left)
            if self.frames.is_empty() {
                 return VmResult::Complete(self.pop().unwrap_or(Value::Null));
            }

            // Get current instruction from top frame
            let frame_idx = self.frames.len() - 1;
            let frame = &mut self.frames[frame_idx];
            
            if frame.ip >= frame.code.len() {
                // End of current frame code
                if self.frames.len() == 1 {
                    // Root frame finished
                    return VmResult::Complete(self.pop().unwrap_or(Value::Null));
                } else {
                    // Implicit return from closure
                    self.frames.pop();
                    // Restore env
                    if let Some(caller) = self.frames.last() {
                        self.runtime.env = caller.env.clone();
                    }
                    self.push(Value::Null);
                    continue;
                }
            }

            let instr = frame.code[frame.ip].clone();
            frame.ip += 1; // Advance IP

            match instr {
                Instr::PushNull => self.push(Value::Null),
                Instr::PushTrue => self.push(Value::Bool(true)),
                Instr::PushFalse => self.push(Value::Bool(false)),
                Instr::PushNum(n) => self.push(Value::Num(n)),
                Instr::PushStr(s) => self.push(Value::Str(s)),
                Instr::Pop => {
                    self.pop();
                }
                Instr::Load(name) => {
                    match self.runtime.get_env(&name) {
                        Some(v) => self.push(v),
                        None => {
                             self.push(Value::Null);
                        }
                    }
                }
                Instr::Store(name) => {
                    let val = self.peek().unwrap_or(&Value::Null).clone();
                    self.runtime.push_env(name, val);
                }
                Instr::Recall => {
                    let key = self.pop().unwrap_or(Value::Null);
                    let val = self.runtime.recall(&key);
                    self.push(val);
                }
                Instr::Remember => {
                    let val = self.pop().unwrap_or(Value::Null);
                    let key = self.pop().unwrap_or(Value::Null);
                    let _ = self.runtime.remember(key, val);
                }
                Instr::Add => {
                    let b = self.pop().unwrap_or(Value::Null);
                    let a = self.pop().unwrap_or(Value::Null);
                    let r = add_values(&a, &b);
                    self.push(r);
                }
                Instr::Mul => {
                    let b = self.pop().unwrap_or(Value::Null);
                    let a = self.pop().unwrap_or(Value::Null);
                    let r = mul_values(&a, &b);
                    self.push(r);
                }
                Instr::Eq => {
                    let b = self.pop().unwrap_or(Value::Null);
                    let a = self.pop().unwrap_or(Value::Null);
                    self.push(Value::Bool(values_equal(&a, &b)));
                }
                Instr::Ne => {
                    let b = self.pop().unwrap_or(Value::Null);
                    let a = self.pop().unwrap_or(Value::Null);
                    self.push(Value::Bool(!values_equal(&a, &b)));
                }
                Instr::Not => {
                    let v = self.pop().unwrap_or(Value::Null);
                    self.push(Value::Bool(v.is_falsy()));
                }
                Instr::And => {
                    let b = self.pop().unwrap_or(Value::Null);
                    let a = self.pop().unwrap_or(Value::Null);
                    self.push(Value::Bool(a.is_truthy() && b.is_truthy()));
                }
                Instr::Or => {
                    let b = self.pop().unwrap_or(Value::Null);
                    let a = self.pop().unwrap_or(Value::Null);
                    self.push(Value::Bool(a.is_truthy() || b.is_truthy()));
                }
                Instr::Jump(target) => {
                    self.frames[frame_idx].ip = target as usize;
                }
                Instr::JumpIfFalse(target) => {
                    let v = self.pop().unwrap_or(Value::Null);
                    if v.is_falsy() {
                        self.frames[frame_idx].ip = target as usize;
                    }
                }
                Instr::JumpIfTrue(target) => {
                    let v = self.pop().unwrap_or(Value::Null);
                    if v.is_truthy() {
                        self.frames[frame_idx].ip = target as usize;
                    }
                }
                Instr::ContextAppend => {
                    let v = self.pop().unwrap_or(Value::Null);
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
                        handlers: Vec::new(), // New frame starts with empty handlers? Yes, try/catch is scoped to block?
                        // Actually, if we are in a block, we might want to inherit handlers?
                        // But EnterTurn is for "Turn" blocks which are closures/scopes.
                        // Exceptions don't propagate *out* of a Turn block to the parent automatically?
                        // Wait, they should bubble up the call stack.
                        // So if this frame has no handler, we pop frame and check caller.
                        // So `handlers` here is strictly *local* handlers in this frame.
                    });
                }
                Instr::PushHandler(offset) => {
                    self.frames[frame_idx].handlers.push(offset);
                }
                Instr::PopHandler => {
                    self.frames[frame_idx].handlers.pop();
                }
                Instr::Throw => {
                    let err = self.pop().unwrap_or(Value::Null);
                    // Unwind
                    loop {
                        if self.frames.is_empty() {
                            // Uncaught exception at top level
                            // For now, return the error as the result (or wrap in Error?)
                            // Ideally we'd return VmResult::Error(err).
                            // But signature is VmResult::Complete(Value).
                            // Let's return the error value.
                            return VmResult::Complete(err);
                        }
                        
                        let frame_idx = self.frames.len() - 1;
                        if let Some(handler_offset) = self.frames[frame_idx].handlers.pop() {
                            // Found handler in current frame
                            self.frames[frame_idx].ip = handler_offset as usize;
                            self.push(err);
                            break;
                        } else {
                            // No handler in this frame, pop frame
                            self.frames.pop();
                            // Restore env if there is a caller
                            if let Some(caller) = self.frames.last() {
                                self.runtime.env = caller.env.clone();
                            }
                        }
                    }
                }
                Instr::CallTool => {
                    let arg = self.pop().unwrap_or(Value::Null);
                    let target_val = self.pop().unwrap_or(Value::Null);
                    
                    match target_val {
                        Value::Str(name) => {
                            // Suspend
                            self.frames[frame_idx].env = self.runtime.env.clone();
                            
                            let continuation = VmState {
                                frames: self.frames.clone(),
                                stack: self.stack.clone(),
                                runtime: self.runtime.clone(),
                            };

                            return VmResult::Suspended {
                                tool_name: name,
                                arg,
                                continuation,
                            };
                        }
                        Value::Closure { code, ip, env } => {
                            // Inject args into memory
                            if let Value::Map(m) = arg {
                                for (k, v) in m {
                                    let _ = self.runtime.remember(Value::Str(k), v);
                                }
                            } else if !arg.is_falsy() {
                                let _ = self.runtime.remember(Value::Str("arg".to_string()), arg);
                            }
                            
                            // Save current env to current frame
                            self.frames[frame_idx].env = self.runtime.env.clone();
                            
                            // Switch to closure env
                            self.runtime.env = env.clone();
                            
                            // Push new Frame
                            self.frames.push(Frame {
                                code,
                                ip,
                                env: self.runtime.env.clone(),
                                handlers: Vec::new(),
                            });
                        }
                        _ => {
                            self.push(Value::Null);
                        }
                    }
                }
                Instr::Return => {
                    let ret_val = self.pop().unwrap_or(Value::Null);
                    if self.frames.len() > 1 {
                        self.frames.pop();
                        // Restore env
                        if let Some(caller) = self.frames.last() {
                            self.runtime.env = caller.env.clone();
                        }
                        self.push(ret_val);
                    } else {
                        return VmResult::Complete(ret_val);
                    }
                }
                Instr::MakeList(count) => {
                    let mut items = Vec::new();
                    for _ in 0..count {
                        items.push(self.pop().unwrap_or(Value::Null));
                    }
                    items.reverse();
                    self.push(Value::List(items));
                }
                Instr::MakeMap(count) => {
                    let mut map = IndexMap::new();
                    for _ in 0..count {
                        let val = self.pop().unwrap_or(Value::Null);
                        let key_val = self.pop().unwrap_or(Value::Null);
                        let key = match key_val {
                            Value::Str(s) => s,
                            _ => key_val.to_string(),
                        };
                        map.insert(key, val);
                    }
                    self.push(Value::Map(map));
                }
                Instr::Index => {
                    let index = self.pop().unwrap_or(Value::Null);
                    let target = self.pop().unwrap_or(Value::Null);
                    let result = match target {
                        Value::List(l) => {
                            if let Value::Num(n) = index {
                                let idx = n as usize;
                                l.get(idx).cloned().unwrap_or(Value::Null)
                            } else {
                                Value::Null
                            }
                        },
                        Value::Map(m) => {
                            let key = match index {
                                Value::Str(s) => s,
                                Value::Num(n) => n.to_string(),
                                _ => "".to_string(),
                            };
                            m.get(&key).cloned().unwrap_or(Value::Null)
                        },
                        _ => Value::Null,
                    };
                    self.push(result);
                }
                Instr::MakeTurn(offset) => {
                    // Capture current code segment and offset AND ENV
                    let current_code = self.frames[frame_idx].code.clone();
                    let current_env = self.runtime.env.clone();
                    self.push(Value::Closure {
                        code: current_code,
                        ip: offset as usize,
                        env: current_env,
                    });
                }
                Instr::LoadModule => {
                    let path_val = self.pop().unwrap_or(Value::Null);
                    let path = match path_val {
                        Value::Str(s) => s,
                        _ => "".to_string(),
                    };

                    // Save env before suspending
                    self.frames[frame_idx].env = self.runtime.env.clone();

                    let continuation = VmState {
                        frames: self.frames.clone(),
                        stack: self.stack.clone(),
                        runtime: self.runtime.clone(),
                    };

                    return VmResult::Suspended {
                        tool_name: "sys_import".to_string(),
                        arg: Value::Str(path),
                        continuation,
                    };
                }
            }
        }
    }

    fn push(&mut self, v: Value) {
        self.stack.push(v);
    }

    fn pop(&mut self) -> Option<Value> {
        self.stack.pop()
    }

    fn peek(&self) -> Option<&Value> {
        self.stack.last()
    }
}

fn add_values(a: &Value, b: &Value) -> Value {
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => Value::Num(x + y),
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
        _ => Value::Str(format!("{}{}", a, b)),
    }
}

fn mul_values(a: &Value, b: &Value) -> Value {
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => Value::Num(x * y),
        _ => Value::Null,
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    a == b
}
