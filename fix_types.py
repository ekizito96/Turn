import re

with open("impl/src/tools.rs", "r") as f:
    code = f.read()

# Fix inference to u64
code = code.replace(", 0))", ", 0u64))")

with open("impl/src/tools.rs", "w") as f:
    f.write(code)

with open("impl/src/lib.rs", "r") as f:
    lib_code = f.read()

lib_code = lib_code.replace("vm = vm::Vm::resume_with_result(continuation, result);", """
                        let mut state = continuation;
                        state.gas_remaining = state.gas_remaining.saturating_sub(cost);
                        vm = vm::Vm::resume_with_result(state, result);
""")

# We also need to fix lib.rs matching Ok((result, cost))
lib_code = lib_code.replace("""match env.tools.call(&tool_name, arg) {
                    Ok(result) => {""", """match env.tools.call(&tool_name, arg) {
                    Ok((result, cost)) => {""")

with open("impl/src/lib.rs", "w") as f:
    f.write(lib_code)

with open("impl/src/runner.rs", "r") as f:
    runner = f.read()
    
runner = runner.replace("state.gas_remaining -= cost;", "state.gas_remaining = state.gas_remaining.saturating_sub(cost);")

with open("impl/src/runner.rs", "w") as f:
    f.write(runner)
