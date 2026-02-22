#!/usr/bin/env python3
"""
Python: Weather agent using Azure OpenAI + weather tool.
Same logic as Turn weather_agent.tn - manual agent loop.

Run: python examples/turn-vs-python/weather_agent.py
Env: AZURE_OPENAI_ENDPOINT, AZURE_OPENAI_API_KEY, AZURE_OPENAI_DEPLOYMENT
"""

import json
import os
import requests


def fetch_weather(lat: float, lon: float) -> str:
    """Open-Meteo (free, no API key)."""
    url = (
        f"https://api.open-meteo.com/v1/forecast"
        f"?latitude={lat}&longitude={lon}&current_weather=true"
    )
    r = requests.get(url)
    if not r.ok:
        return json.dumps({"error": r.text})
    j = r.json()
    cw = j.get("current_weather", {})
    temp = cw.get("temperature", 0)
    code = cw.get("weathercode", 0)
    desc = _weather_code_to_desc(code)
    return json.dumps({"temp": temp, "conditions": desc})


def _weather_code_to_desc(code: int) -> str:
    if code == 0:
        return "clear"
    if code in (1, 2, 3):
        return "partly cloudy"
    if code in (45, 48):
        return "foggy"
    if 51 <= code <= 67:
        return "rainy"
    if 71 <= code <= 77:
        return "snowy"
    if 80 <= code <= 82:
        return "rain showers"
    if code in (85, 86):
        return "snow showers"
    if 95 <= code <= 99:
        return "thunderstorm"
    return "unknown"


def run_agent(question: str) -> str:
    """Agent loop: Azure OpenAI + tool calling."""
    endpoint = os.environ.get("AZURE_OPENAI_ENDPOINT", "").rstrip("/")
    api_key = os.environ.get("AZURE_OPENAI_API_KEY", "")
    deployment = os.environ.get("AZURE_OPENAI_DEPLOYMENT", "gpt-4o")

    if not endpoint or not api_key:
        return "Set AZURE_OPENAI_ENDPOINT, AZURE_OPENAI_API_KEY, AZURE_OPENAI_DEPLOYMENT"

    url = f"{endpoint}/openai/deployments/{deployment}/chat/completions?api-version=2024-10-21"

    tools_schema = [
        {
            "type": "function",
            "function": {
                "name": "get_weather",
                "description": "Get current weather for a location by latitude and longitude. Use latitude 37.77 and longitude -122.42 for San Francisco.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "latitude": {"type": "number", "description": "Latitude"},
                        "longitude": {"type": "number", "description": "Longitude"},
                    },
                    "required": ["latitude", "longitude"],
                },
            },
        }
    ]

    messages = [
        {
            "role": "system",
            "content": "You are a helpful weather assistant. When the user asks about weather, use the get_weather tool with latitude and longitude. Respond concisely.",
        },
        {"role": "user", "content": question},
    ]

    for _ in range(5):
        body = {
            "messages": messages,
            "max_tokens": 500,
            "tools": tools_schema,
            "tool_choice": "auto",
        }

        r = requests.post(
            url,
            headers={"api-key": api_key, "Content-Type": "application/json"},
            json=body,
        )

        if not r.ok:
            return f"API error {r.status_code}: {r.text}"

        j = r.json()
        choice = j.get("choices", [{}])[0]
        msg = choice.get("message", {})

        messages.append(msg)

        tool_calls = msg.get("tool_calls") or []
        if not tool_calls:
            return msg.get("content", "")

        for tc in tool_calls:
            name = tc.get("function", {}).get("name", "")
            args_str = tc.get("function", {}).get("arguments", "{}")
            tc_id = tc.get("id", "")

            if name == "get_weather":
                args = json.loads(args_str)
                lat = args.get("latitude", 37.77)
                lon = args.get("longitude", -122.42)
                result = fetch_weather(lat, lon)
                messages.append(
                    {"role": "tool", "tool_call_id": tc_id, "content": result}
                )

    return "Max turns reached"


if __name__ == "__main__":
    question = "What's the weather in San Francisco?"
    answer = run_agent(question)
    print(answer)
