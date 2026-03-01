import re

with open("impl/src/vm.rs", "r") as f:
    code = f.read()

def replace_in_func(func_name, old, new, text):
    start = text.find(f"fn {func_name}")
    if start == -1: return text
    
    # Extract the function body
    end_func = text.find("\n}\n", start) + 3
    func_body = text[start:end_func]
    
    # Replace inside the function body
    func_body = func_body.replace(old, new)
    
    # Reassemble
    return text[:start] + func_body + text[end_func:]

for func in ["add_values", "sub_values", "mul_values", "div_values"]:
    code = replace_in_func(func, "(*p1).min(*p2).min(p3)", "p1 * p2 * p3", code)
    code = replace_in_func(func, "(*p1).min(*p2)", "p1 * p2", code)
    code = replace_in_func(func, "(*p).min(p2)", "p * p2", code)

with open("impl/src/vm.rs", "w") as f:
    f.write(code)
