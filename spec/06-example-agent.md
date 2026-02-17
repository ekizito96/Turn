# Example: Working Agent in Turn

**Status:** Reference example showing a realistic agent workflow. Uses the v1 minimal core: multiple turns, context, memory, tool calls, and control flow.

---

## The Agent: Task Assistant

This agent receives a task, uses tools to gather information, remembers key facts, makes decisions, and returns a result. It demonstrates:

- **Multiple turns** (agent runs several steps)
- **Context management** (appending messages/state)
- **Memory** (remembering facts, recalling them later)
- **Tool calls** (calling external tools and using results)
- **Control flow** (if statements, conditional behavior)

---

## The Program

```turn
// Task Assistant Agent
// Receives a task, gathers info, remembers facts, makes decisions

turn {
  // Receive initial task
  let task = "Find the weather in San Francisco and book a flight if it's sunny";
  remember("task", task);
  context.append(task);
  
  // Step 1: Get weather
  let weather_result = call("get_weather", "San Francisco");
  remember("weather", weather_result);
  context.append(weather_result);
  
  // Step 2: Parse weather result (tool returns "sunny" or "rainy")
  let weather_str = recall("weather");
  let is_sunny = call("parse_weather", weather_str);
  remember("is_sunny", is_sunny);
  
  // Decision: book flight only if sunny
  // Note: In v1, we compare strings. Assume parse_weather returns "true" or "false"
  if is_sunny {
    turn {
      // Book flight
      let flight_result = call("book_flight", "San Francisco");
      remember("flight", flight_result);
      context.append(flight_result);
      return "Task complete: Weather is sunny, flight booked";
    }
  } else {
    turn {
      // Don't book flight
      context.append("Weather not sunny, skipping flight");
      return "Task complete: Weather not sunny, no flight booked";
    }
  }
}

// Second turn: Follow-up action
turn {
  // Recall what we did
  let previous_task = recall("task");
  let weather = recall("weather");
  let flight = recall("flight");
  
  // Generate summary (tool handles formatting)
  let summary = call("generate_summary", previous_task);
  context.append(summary);
  
  return summary;
}
```

---

## Expected Behavior (Trace)

**Turn 1:**
1. Agent receives task: `"Find the weather in San Francisco and book a flight if it's sunny"`
2. Stores task in memory: `memory["task"] = "Find the weather..."`
3. Appends task to context: `context = ["Find the weather..."]`
4. Calls `get_weather("San Francisco")` → **suspends**
   - Runtime invokes weather tool handler
   - Handler returns `"sunny"`
   - **Resumes**; `weather_result = "sunny"`
5. Stores weather: `memory["weather"] = "sunny"`
6. Appends weather to context: `context = ["Find the weather...", "sunny"]`
7. Recalls weather from memory: `weather_str = "sunny"`
8. Calls `parse_weather("sunny")` → **suspends** → **resumes** with `is_sunny = "true"` (string)
9. Stores `is_sunny = "true"`
10. Evaluates `if is_sunny { ... }` → condition is truthy (non-empty string), enters nested turn
11. **Nested turn:** Calls `book_flight("San Francisco")` → **suspends** → **resumes** with `flight_result = "Flight ABC123 booked"`
12. Stores flight: `memory["flight"] = "Flight ABC123 booked"`
13. Appends flight result to context: `context = [..., "Flight ABC123 booked"]`
14. Returns: `"Task complete: Weather is sunny, flight booked"`
15. Turn 1 completes

**Turn 2:**
1. Agent starts new turn
2. Recalls from memory: `previous_task = "Find the weather..."`, `weather = "sunny"`
3. Calls `generate_summary(...)` → **suspends** → **resumes** with summary
4. Appends summary to context
5. Returns summary
6. Turn 2 completes

**Final state:**
- `context`: `["Find the weather...", "sunny", "Flight ABC123 booked", "<summary>"]`
- `memory`: `{"task": "Find the weather...", "weather": "sunny", "is_sunny": "true", "flight": "Flight ABC123 booked"}`
- Both turns completed successfully

---

## Key Patterns Demonstrated

1. **Multi-turn workflow:** Agent runs multiple turns; each turn is a checkpointable unit.

2. **Context accumulation:** Each turn appends to context, building a history. Context is bounded (runtime enforces limit).

3. **Memory persistence:** Facts stored with `remember` persist across turns; `recall` retrieves them later.

4. **Tool calls:** `call(tool_name, arg)` suspends execution; runtime invokes handler; execution resumes with result. Both statement (`call(...);`) and expression (`let x = call(...)`) forms shown.

5. **Conditional behavior:** `if` statement branches based on tool results or memory state.

6. **Nested turns:** A turn can contain another turn (e.g., the `if` branches each contain a turn). This allows checkpointing at decision points.

---

## Tools Used (Runtime Registry)

The runtime must provide these tools (or the example assumes they exist):

- `get_weather(location: string) → string` — Returns weather condition
- `parse_weather(weather: string) → bool` — Parses weather string, returns true if sunny
- `book_flight(destination: string) → string` — Books flight, returns confirmation
- `generate_summary(task: string) → string` — Generates summary of completed task

In a real implementation, these would be registered in the runtime's tool registry (see [03-runtime-model.md](03-runtime-model.md) §5).

---

## Notes

- **v1 compliance:** This example uses only v1 syntax: no string concatenation (`+`), no infix operators. Tools handle formatting/concatenation if needed (e.g., `generate_summary` formats the output).
- **Boolean values:** v1 has numbers and strings. The example uses `"true"`/`"false"` strings for booleans; `if` treats non-empty strings as truthy. Alternatively, use `"1"`/`"0"` or numbers.
- **Nested turns:** The `if` branches contain turns. This is valid: a turn can contain statements, including other turns. The inner turn completes before the outer turn continues (or the outer turn completes if the inner turn returns).
- **Context entries:** Each `context.append(...)` adds one entry. In a real agent, tools might format messages (e.g., `"Weather: sunny"`), but the agent code just appends values.

This example shows how Turn's primitives compose to build a working agent that uses tools, manages context and memory, and makes decisions across multiple turns.
