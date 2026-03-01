import re

with open("impl/src/tools.rs", "r") as f:
    code = f.read()

# 1. Update ToolHandler signature
code = code.replace(
    "pub type ToolHandler = Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync>;",
    "pub type ToolHandler = Box<dyn Fn(Value) -> Result<(Value, u64), String> + Send + Sync>;"
)

# 2. Update simple tools to return Ok((val, 0))
# Let's do a simple regex for Ok(expr) -> Ok((expr, 0)) but ONLY for the tool handlers
# A safer way is to replace Ok(...) within the tool handler closures.
# We'll replace Ok(Value::Null) with Ok((Value::Null, 0))
code = code.replace("Ok(Value::Null)", "Ok((Value::Null, 0))")
code = code.replace("Ok(arg)", "Ok((arg, 0))")
code = code.replace("Ok(Value::Str(content))", "Ok((Value::Str(content), 0))")
code = code.replace("Ok(Value::Str(text))", "Ok((Value::Str(text), 0))")
code = code.replace("Ok(Value::Str(val))", "Ok((Value::Str(val), 0))")
code = code.replace("Ok(Value::Str(s))", "Ok((Value::Str(s), 0))")
code = code.replace("Ok(Value::Num(now.as_secs_f64()))", "Ok((Value::Num(now.as_secs_f64()), 0))")
code = code.replace("Ok(Value::Bool(re.is_match(&text)))", "Ok((Value::Bool(re.is_match(&text)), 0))")
code = code.replace("Ok(Value::Str(re.replace_all(&text, replacement.as_str()).to_string()))", "Ok((Value::Str(re.replace_all(&text, replacement.as_str()).to_string()), 0))")
code = code.replace("Ok(v)", "Ok((v, 0))")

# For llm_generate and llm_infer, we need to handle the token count properly.
# Currently call_llm_dispatch returns Result<String, String>. We need to change it to Result<(String, u64), String>.

# Let's do the manual replacements for llm dispatch
def replace_openai(text):
    old = """                        if let Some(content) = json["choices"][0]["message"]["content"].as_str() {
                            Ok(content.to_string())
                        } else {"""
    new = """                        if let Some(content) = json["choices"][0]["message"]["content"].as_str() {
                            let tokens = json["usage"]["total_tokens"].as_u64().unwrap_or(0);
                            Ok((content.to_string(), tokens))
                        } else {"""
    return text.replace(old, new)

def replace_anthropic(text):
    old = """                            if let Some(text) = content_arr.first().and_then(|item| item["text"].as_str()) {
                                Ok(text.to_string())
                            } else {"""
    new = """                            if let Some(text) = content_arr.first().and_then(|item| item["text"].as_str()) {
                                let input_tok = json["usage"]["input_tokens"].as_u64().unwrap_or(0);
                                let output_tok = json["usage"]["output_tokens"].as_u64().unwrap_or(0);
                                Ok((text.to_string(), input_tok + output_tok))
                            } else {"""
    return text.replace(old, new)

def replace_gemini(text):
    old = """                                    if let Some(text) = parts.first().and_then(|p| p["text"].as_str()) {
                                        return Ok(text.to_string());
                                    }"""
    new = """                                    if let Some(text) = parts.first().and_then(|p| p["text"].as_str()) {
                                        let tokens = json["usageMetadata"]["totalTokenCount"].as_u64().unwrap_or(0);
                                        return Ok((text.to_string(), tokens));
                                    }"""
    return text.replace(old, new)

def replace_ollama(text):
    old = """                        if let Some(content) = json["message"]["content"].as_str() {
                            Ok(content.to_string())
                        } else {"""
    new = """                        if let Some(content) = json["message"]["content"].as_str() {
                            let tokens = json["eval_count"].as_u64().unwrap_or(0) + json["prompt_eval_count"].as_u64().unwrap_or(0);
                            Ok((content.to_string(), tokens))
                        } else {"""
    return text.replace(old, new)

def replace_azure_chat(text):
    old = """                        if let Some(content) = json["choices"][0]["message"]["content"].as_str() {
                            Ok(content.to_string())
                        } else {"""
    new = """                        if let Some(content) = json["choices"][0]["message"]["content"].as_str() {
                            let tokens = json["usage"]["total_tokens"].as_u64().unwrap_or(0);
                            Ok((content.to_string(), tokens))
                        } else {"""
    # Already matched by replace_openai partially if they are identical strings, but let's be careful.
    return text.replace(old, new)

def replace_azure_responses(text):
    old = """                        if let Some(text) = json.get("output_text").and_then(|v| v.as_str()) {
                            return Ok(text.to_string());
                        }
                        if let Some(text) = json["output"][0]["content"][0]["text"].as_str() {
                            return Ok(text.to_string());
                        }
                        if let Some(text) = json["choices"][0]["message"]["content"].as_str() {
                            return Ok(text.to_string());
                        }"""
    new = """                        let tokens = json["usage"]["total_tokens"].as_u64().unwrap_or(0);
                        if let Some(text) = json.get("output_text").and_then(|v| v.as_str()) {
                            return Ok((text.to_string(), tokens));
                        }
                        if let Some(text) = json["output"][0]["content"][0]["text"].as_str() {
                            return Ok((text.to_string(), tokens));
                        }
                        if let Some(text) = json["choices"][0]["message"]["content"].as_str() {
                            return Ok((text.to_string(), tokens));
                        }"""
    return text.replace(old, new)


code = replace_openai(code)
code = replace_anthropic(code)
code = replace_gemini(code)
code = replace_ollama(code)
code = replace_azure_responses(code)

code = code.replace("-> Result<String, String>", "-> Result<(String, u64), String>")

# In llm_generate
code = code.replace("""                match call_llm_dispatch(model_opt.as_deref(), &json_msgs) {
                    Ok((content, 0)) => Ok((Value::Str(content), 0)),
                    Err(e) => Err(e),
                }""", """                match call_llm_dispatch(model_opt.as_deref(), &json_msgs) {
                    Ok((content, tokens)) => Ok((Value::Str(content), tokens)),
                    Err(e) => Err(e),
                }""")
code = code.replace("""                match call_llm_dispatch(model_opt.as_deref(), &json_msgs) {
                    Ok(content) => Ok((Value::Str(content), 0)),
                    Err(e) => Err(e),
                }""", """                match call_llm_dispatch(model_opt.as_deref(), &json_msgs) {
                    Ok((content, tokens)) => Ok((Value::Str(content), tokens)),
                    Err(e) => Err(e),
                }""")

# In llm_infer
code = code.replace("""                     match call_llm_dispatch(None, &messages) {
                         Ok(content) => {
                             let clean = content.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();""", """                     match call_llm_dispatch(None, &messages) {
                         Ok((content, tokens)) => {
                             let clean = content.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();""")

code = code.replace("""                                     Ok((Value::Uncertain(Box::new(turn_val), conf), 0))
                                 },
                                 Err(e) => Err(format!("Failed to parse LLM JSON: {} in '{}'", e, clean)),
                             }
                         },
                         Err(_) => {""", """                                     Ok((Value::Uncertain(Box::new(turn_val), conf), tokens))
                                 },
                                 Err(e) => Err(format!("Failed to parse LLM JSON: {} in '{}'", e, clean)),
                             }
                         },
                         Err(_) => {""")

code = code.replace("""                                     Ok(Value::Uncertain(Box::new(turn_val), conf))
                                 },
                                 Err(e) => Err(format!("Failed to parse LLM JSON: {} in '{}'", e, clean)),
                             }
                         },
                         Err(_) => {""", """                                     Ok((Value::Uncertain(Box::new(turn_val), conf), tokens))
                                 },
                                 Err(e) => Err(format!("Failed to parse LLM JSON: {} in '{}'", e, clean)),
                             }
                         },
                         Err(_) => {""")

code = code.replace("""                             match schema {
                                 Value::Str(s) if s.contains("Num") => {
                                     Ok((Value::Uncertain(Box::new(Value::Num(42.0)), 0.85), 0))
                                 }
                                 Value::Str(s) if s.contains("Bool") => {
                                     Ok((Value::Uncertain(Box::new(Value::Bool(true)), 0.9), 0))
                                 }
                                 Value::Str(s) if s.contains("Str") => {
                                     Ok((Value::Uncertain(Box::new(Value::Str("Mock Response".to_string())), 0.7), 0))
                                 }
                                 _ => Ok((Value::Uncertain(Box::new(Value::Null), 0.5), 0)),
                             }""", """                             match schema {
                                 Value::Str(s) if s.contains("Num") => {
                                     Ok((Value::Uncertain(Box::new(Value::Num(42.0)), 0.85), 0))
                                 }
                                 Value::Str(s) if s.contains("Bool") => {
                                     Ok((Value::Uncertain(Box::new(Value::Bool(true)), 0.9), 0))
                                 }
                                 Value::Str(s) if s.contains("Str") => {
                                     Ok((Value::Uncertain(Box::new(Value::Str("Mock Response".to_string())), 0.7), 0))
                                 }
                                 _ => Ok((Value::Uncertain(Box::new(Value::Null), 0.5), 0)),
                             }""")


with open("impl/src/tools.rs", "w") as f:
    f.write(code)
