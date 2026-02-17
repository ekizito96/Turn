//! Tool registry and handlers. Per spec: at least `echo` built-in.

use crate::value::Value;
use std::collections::HashMap;

pub type ToolHandler = Box<dyn Fn(Value) -> Value + Send + Sync>;

pub struct ToolRegistry {
    tools: HashMap<String, ToolHandler>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut tools = HashMap::new();
        tools.insert("echo".to_string(), Box::new(|arg| arg) as ToolHandler);
        Self { tools }
    }

    pub fn register(&mut self, name: impl Into<String>, handler: ToolHandler) {
        self.tools.insert(name.into(), handler);
    }

    pub fn call(&self, name: &str, arg: Value) -> Option<Value> {
        self.tools.get(name).map(|h| h(arg))
    }

    pub fn has(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }
}
