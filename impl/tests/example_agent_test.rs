//! Integration test: run example agent from spec/06-example-agent.md

use turn::run_with_tools;
use turn::tools::{ToolHandler, ToolRegistry};
use turn::value::Value;

fn example_agent_tools() -> ToolRegistry {
    let mut tools = ToolRegistry::new();
    tools.register(
        "get_weather",
        Box::new(|arg: Value| {
            let s = arg.to_string();
            if s.contains("San Francisco") {
                Value::Str("sunny".to_string())
            } else {
                Value::Str("rainy".to_string())
            }
        }) as ToolHandler,
    );
    tools.register(
        "parse_weather",
        Box::new(|arg: Value| {
            let s = arg.to_string();
            Value::Str(if s == "sunny" { "true" } else { "false" }.to_string())
        }) as ToolHandler,
    );
    tools.register(
        "book_flight",
        Box::new(|arg: Value| Value::Str(format!("Flight ABC123 booked to {}", arg)))
            as ToolHandler,
    );
    tools.register(
        "generate_summary",
        Box::new(|arg: Value| Value::Str(format!("Summary: {}", arg))) as ToolHandler,
    );
    tools
}

#[test]
fn example_agent_runs_to_completion() {
    let source = r#"
// Task Assistant Agent
turn {
  let task = "Find the weather in San Francisco and book a flight if it's sunny";
  remember("task", task);
  context.append("Task: " + task);

  let weather_result = call("get_weather", "San Francisco");
  remember("weather", weather_result);
  context.append("Weather: " + weather_result);

  let weather_str = recall("weather");
  let is_sunny = call("parse_weather", weather_str);
  remember("is_sunny", is_sunny);

  if is_sunny {
    turn {
      let flight_result = call("book_flight", "San Francisco");
      remember("flight", flight_result);
      context.append("Flight: " + flight_result);
      return "Task complete: Weather is sunny, flight booked";
    }
  } else {
    turn {
      context.append("Weather not sunny, skipping flight");
      return "Task complete: Weather not sunny, no flight booked";
    }
  }
}

turn {
  let previous_task = recall("task");
  let weather = recall("weather");
  let flight = recall("flight");

  let summary = call("generate_summary", previous_task);
  context.append(summary);

  return summary;
}
"#;
    let tools = example_agent_tools();
    let result = run_with_tools(source, &tools).unwrap();
    assert!(result.to_string().starts_with("Summary:"));
}
