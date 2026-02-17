//! Bytecode VM for Turn.

use crate::bytecode::Instr;
use crate::runtime::Runtime;
use crate::tools::ToolRegistry;
use crate::value::Value;

#[derive(Debug)]
pub enum VmResult {
    Complete(Value),
    Suspended {
        tool_name: String,
        arg: Value,
        continuation: VmState,
    },
}

#[derive(Debug, Clone)]
pub struct VmState {
    pub code: Vec<Instr>,
    pub ip: usize,
    pub stack: Vec<Value>,
    pub runtime: Runtime,
}

pub struct Vm<'a> {
    code: &'a [Instr],
    ip: usize,
    stack: Vec<Value>,
    return_addrs: Vec<usize>,
    result: Value,
    runtime: Runtime,
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
        loop {
            let instr = match self.fetch() {
                Some(i) => i.clone(),
                None => break,
            };
            self.ip += 1;

            match instr {
                Instr::PushNum(n) => self.push(Value::Num(n)),
                Instr::PushStr(s) => self.push(Value::Str(s)),
                Instr::PushTrue => self.push(Value::Bool(true)),
                Instr::PushFalse => self.push(Value::Bool(false)),
                Instr::PushNull => self.push(Value::Null),

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
                    if let Some(result) = self.tools.call(&name, arg) {
                        self.push(result);
                    } else {
                        self.push(Value::Null);
                    }
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
        (Value::Str(_), _) | (_, Value::Str(_)) => Value::Str(format!("{}{}", a, b)),
        (Value::Num(x), Value::Num(y)) => Value::Num(x + y),
        _ => Value::Str(format!("{}{}", a, b)),
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Null, Value::Null) => true,
        _ => false,
    }
}
