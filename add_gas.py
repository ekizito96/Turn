import re

with open("impl/src/vm.rs", "r") as f:
    code = f.read()

# 1. Update VmState struct
code = code.replace(
    "pub mailbox: VecDeque<Value>,",
    "pub mailbox: VecDeque<Value>,\n    #[serde(default = \"default_gas\")]\n    pub gas_remaining: u64,"
)

# 2. Add default_gas function
if "fn default_gas() -> u64" not in code:
    code = code.replace(
        "pub enum VmResult {",
        "fn default_gas() -> u64 { 1_000_000 }\n\n#[derive(Debug)]\npub enum VmResult {"
    )

# 3. Update Vm::new
code = code.replace(
    "runtime: Runtime::new(),\n            mailbox: VecDeque::new(),\n        };",
    "runtime: Runtime::new(),\n            mailbox: VecDeque::new(),\n            gas_remaining: default_gas(),\n        };"
)

# 4. Update Vm::resume_with_result
code = code.replace(
    "mailbox: state.mailbox,\n        };",
    "mailbox: state.mailbox,\n            gas_remaining: state.gas_remaining,\n        };"
)

# 5. Update Vm::resume_with_error
code = code.replace(
    "mailbox: state.mailbox,\n            107|        };",
    "mailbox: state.mailbox,\n            gas_remaining: state.gas_remaining,\n        };"
)
# Ah wait, line 107 might be tricky, let's use regex for Process creation in resume_with_error and resume_with_result
