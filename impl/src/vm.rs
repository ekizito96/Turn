//! Bytecode VM for Turn.

use crate::bytecode::Instr;
use crate::runtime::Runtime;
use crate::tools::ToolRegistry;
use crate::value::Value;
use indexmap::IndexMap;

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum VmResult {
    Complete(Value),
    Suspended {
        tool_name: String,
        arg: Value,
        continuation: VmState,
    },
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmState {
    pub code: Vec<Instr>,
    pub ip: usize,
    pub stack: Vec<Value>,
    pub return_addrs: Vec<usize>,
    pub runtime: Runtime,
}

pub struct Vm<'a> {
    code: &'a [Instr],
    ip: usize,
    stack: Vec<Value>,
    return_addrs: Vec<usize>,
    result: Value,
    runtime: Runtime,
    #[allow(dead_code)]
    tools: &'a ToolRegistry,
}

impl<'a> Vm<'a> {
    pub fn new(code: &'a [Instr], tools: &'a ToolRegistry) -> Self {
        Self {
            code,
            ip: 0,
            stack: Vec::new(),
            return_addrs: Vec::new(),
            result: Value::Null,
            runtime: Runtime::new(),
            tools,
        }
    }

    pub fn resume_with_result(
        state: VmState,
        code: &'a [Instr],
        tools: &'a ToolRegistry,
        result: Value,
    ) -> Self {
        let mut vm = Self {
            code,
            ip: state.ip,
            stack: state.stack,
            return_addrs: state.return_addrs,
            result: Value::Null,
            runtime: state.runtime,
            tools,
        };
        vm.push(result);
        vm
    }

    fn fetch(&self) -> Option<&Instr> {
        self.code.get(self.ip)
    }

    fn pop(&mut self) -> Option<Value> {
        self.stack.pop()
    }

    fn push(&mut self, v: Value) {
        self.stack.push(v);
    }

    #[allow(dead_code)]
    fn peek(&self, n: usize) -> Option<&Value> {
        let len = self.stack.len();
        if len > n {
            self.stack.get(len - 1 - n)
        } else {
            None
        }
    }

    pub fn run(&mut self) -> VmResult {
        while let Some(i) = self.fetch() {
            let instr = i.clone();
            self.ip += 1;

            match instr {
                Instr::PushNum(n) => self.push(Value::Num(n)),
                Instr::PushStr(s) => self.push(Value::Str(s)),
                Instr::PushTrue => self.push(Value::Bool(true)),
                Instr::PushFalse => self.push(Value::Bool(false)),
                Instr::PushNull => self.push(Value::Null),
                Instr::MakeList(n) => {
                    let mut items = Vec::with_capacity(n);
                    for _ in 0..n {
                        items.push(self.pop().unwrap_or(Value::Null));
                    }
                    items.reverse();
                    self.push(Value::List(items));
                }
                Instr::MakeMap(n) => {
                    let mut entries = Vec::with_capacity(n);
                    for _ in 0..n {
                        let val = self.pop().unwrap_or(Value::Null);
                        let key_val = self.pop().unwrap_or(Value::Null);
                        let key = match key_val {
                            Value::Str(s) => s,
                            _ => key_val.to_string(),
                        };
                        entries.push((key, val));
                    }
                    entries.reverse();
                    let mut map = IndexMap::with_capacity(n);
                    for (k, v) in entries {
                        map.insert(k, v);
                    }
                    self.push(Value::Map(map));
                }

                Instr::Load(name) => {
                    if let Some(v) = self.runtime.get_env(&name) {
                        self.push(v);
                    } else {
                        self.push(Value::Null);
                    }
                }
                Instr::Store(name) => {
                    if let Some(v) = self.pop() {
                        self.runtime.push_env(name, v);
                    }
                }

                Instr::Add => {
                    let b = self.pop().unwrap_or(Value::Null);
                    let a = self.pop().unwrap_or(Value::Null);
                    let r = add_values(&a, &b);
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
                Instr::And => {
                    let b = self.pop().unwrap_or(Value::Null);
                    let a = self.pop().unwrap_or(Value::Null);
                    let r = if a.is_falsy() { a } else { b };
                    self.push(r);
                }
                Instr::Or => {
                    let b = self.pop().unwrap_or(Value::Null);
                    let a = self.pop().unwrap_or(Value::Null);
                    let r = if a.is_truthy() { a } else { b };
                    self.push(r);
                }

                Instr::Pop => {
                    self.pop();
                }

                Instr::ContextAppend => {
                    if let Some(v) = self.pop() {
                        let _ = self.runtime.append_context(v);
                    }
                }
                Instr::Remember => {
                    let val = self.pop().unwrap_or(Value::Null);
                    let key = self.pop().unwrap_or(Value::Null);
                    let _ = self.runtime.remember(key, val);
                }
                Instr::Recall => {
                    let key = self.pop().unwrap_or(Value::Null);
                    let v = self.runtime.recall(&key);
                    self.push(v);
                }
                Instr::CallTool => {
                    let arg = self.pop().unwrap_or(Value::Null);
                    let name_val = self.pop().unwrap_or(Value::Null);
                    let name = match name_val {
                        Value::Str(s) => s,
                        Value::Num(n) => n.to_string(),
                        _ => "".to_string(),
                    };

                    // Suspend execution so host can run the tool (async/sync)
                    let continuation = VmState {
                        code: self.code.to_vec(),
                        ip: self.ip,
                        stack: self.stack.clone(),
                        return_addrs: self.return_addrs.clone(),
                        runtime: self.runtime.clone(),
                    };

                    return VmResult::Suspended {
                        tool_name: name,
                        arg,
                        continuation,
                    };
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

                Instr::Jump(target) => {
                    self.ip = target as usize;
                }
                Instr::JumpIfFalse(target) => {
                    let v = self.pop().unwrap_or(Value::Null);
                    if v.is_falsy() {
                        self.ip = target as usize;
                    }
                }
                Instr::JumpIfTrue(target) => {
                    let v = self.pop().unwrap_or(Value::Null);
                    if v.is_truthy() {
                        self.ip = target as usize;
                    }
                }

                Instr::EnterTurn(after_addr) => {
                    self.return_addrs.push(after_addr as usize);
                }
                Instr::Return => {
                    let value = self.pop().unwrap_or(Value::Null);
                    self.result = value.clone();
                    if let Some(addr) = self.return_addrs.pop() {
                        self.ip = addr;
                    } else {
                        return VmResult::Complete(value);
                    }
                }
            }
        }

        VmResult::Complete(self.result.clone())
    }
}

fn add_values(a: &Value, b: &Value) -> Value {
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => Value::Num(x + y),
        // Lists: concatenation
        (Value::List(l1), Value::List(l2)) => {
            let mut new_list = l1.clone();
            new_list.extend(l2.clone());
            Value::List(new_list)
        },
        // Maps: merge (right overrides left)
        (Value::Map(m1), Value::Map(m2)) => {
            let mut new_map = m1.clone();
            for (k, v) in m2 {
                new_map.insert(k.clone(), v.clone());
            }
            Value::Map(new_map)
        },
        // String concatenation (coercive)
        _ => Value::Str(format!("{}{}", a, b)),
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    a == b
}
