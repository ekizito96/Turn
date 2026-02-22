"""
Python: Same task assistant - what you'd typically write
Run: python examples/turn-vs-python/python_example.py

This shows the boilerplate Turn eliminates.
"""

# --- 1. Context: you build it yourself ---
messages = []
MAX_TOKENS = 4000  # you pick, you enforce


def add_to_context(role: str, content: str):
    messages.append({"role": role, "content": content})
    # In real code: count tokens, trim if over limit, handle priority
    # if count_tokens(messages) > MAX_TOKENS:
    #     messages = trim_oldest(messages, MAX_TOKENS)


# --- 2. Memory: you build it yourself ---
memory = {}


def remember(key: str, value: str):
    memory[key] = value


def recall(key: str):
    return memory.get(key)


# --- 3. Tool registry: you build it yourself ---
def get_weather(location: str):
    return "sunny"  # stub


tools = {"get_weather": get_weather}


def call_tool(name: str, arg: str):
    return tools[name](arg)


# --- 4. The "turn" - you write the loop ---
def run_turn():
    task = "Find weather in San Francisco"
    remember("task", task)
    add_to_context("user", f"Task: {task}")

    weather = call_tool("get_weather", "San Francisco")
    remember("weather", weather)
    add_to_context("assistant", f"Weather: {weather}")

    return weather


if __name__ == "__main__":
    result = run_turn()
    print(result)
