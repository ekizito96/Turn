# Turn vs Python: Same Agent, Side by Side

Same task assistant logic in both languages.

## Turn (11 lines)

```turn
turn {
  let task = "Find weather in San Francisco";
  remember("task", task);
  context.append("Task: " + task);

  let weather = call("get_weather", "San Francisco");
  remember("weather", weather);
  context.append("Weather: " + weather);

  return weather;
}
```

**Primitives:** `turn`, `remember`, `context.append`, `call` — built into the language.

## Python (~60 lines)

You implement:

1. **Context** — list + add_to_context + token counting + trim logic
2. **Memory** — dict + remember/recall helpers
3. **Tool registry** — dict + call_tool
4. **The turn** — your function that does the work

In production: 500+ line ContextManager, 3400+ line agent kernel.

## Run

```bash
# Turn
cd impl && cargo run -- run ../examples/turn-vs-python/turn_example.turn

# Python
python examples/turn-vs-python/python_example.py
```

---

## Weather Agent (Azure OpenAI + LLM)

Real LLM agent using Azure OpenAI with a weather tool. Same logic in Turn and Python.

### Turn (6 lines)

```turn
turn {
  let question = "What's the weather in San Francisco?";
  let answer = call("llm", question);
  return answer;
}
```

### Python (~120 lines)

Manual agent loop: messages, tool schema, tool-call handling, retries.

### Run

```bash
# Set Azure OpenAI credentials
export AZURE_OPENAI_ENDPOINT="https://YOUR_RESOURCE.openai.azure.com/"
export AZURE_OPENAI_API_KEY="your-api-key"
export AZURE_OPENAI_DEPLOYMENT="gpt-4o"   # or your deployment name

# Turn (uses weather + llm tools)
cd impl && cargo run -- run --with-llm ../examples/turn-vs-python/weather_agent.turn

# Python (pip install requests)
python examples/turn-vs-python/weather_agent.py
```
