//! Embedded Standard Library for Turn.

pub fn get_module_source(name: &str) -> Option<&'static str> {
    match name {
        "std/fs" => Some(FS_SOURCE),
        "std/http" => Some(HTTP_SOURCE),
        "std/math" => Some(MATH_SOURCE),
        "std/env" => Some(ENV_SOURCE),
        "std/json" => Some(JSON_SOURCE),
        "std/time" => Some(TIME_SOURCE),
        "std/regex" => Some(REGEX_SOURCE),
        _ => None,
    }
}

const FS_SOURCE: &str = r#"
// File System Module
// Usage: let fs = use "std/fs"; let content = fs.read("file.txt");

let read = turn(path: Str) -> Str {
    return call("fs_read", path);
};

let write = turn(path: Str, content: Str) -> Void {
    call("fs_write", { "path": path, "content": content });
    return null;
};

let read_blob = turn(path: Str) -> Blob {
    return call("fs_read_blob", path);
};

return {
    "read": read,
    "write": write,
    "read_blob": read_blob
};
"#;

const HTTP_SOURCE: &str = r#"
// HTTP Module
// Usage: let http = use "std/http"; let html = http.get("https://example.com");

let get = turn(url: Str) -> Str {
    return call("http_get", url);
};

let post = turn(url: Str, body: Any) -> Str {
    return call("http_post", { "url": url, "body": body });
};

return {
    "get": get,
    "post": post
};
"#;

const MATH_SOURCE: &str = r#"
// Math Module
// Usage: let math = use "std/math";

let max = turn(a: Num, b: Num) -> Num {
    if a > b { return a; } else { return b; }
};

let min = turn(a: Num, b: Num) -> Num {
    if a < b { return a; } else { return b; }
};

let abs = turn(n: Num) -> Num {
    if n < 0 { return -n; } else { return n; }
};

return {
    "max": max,
    "min": min,
    "abs": abs
};
"#;

const ENV_SOURCE: &str = r#"
// Environment Module
// Usage: let env = use "std/env";

let get = turn(key: Str) -> Str {
    return call("env_get", key);
};

let set = turn(key: Str, value: Str) -> Void {
    call("env_set", { "key": key, "value": value });
    return null;
};

return {
    "get": get,
    "set": set
};
"#;

const JSON_SOURCE: &str = r#"
// JSON Module
// Usage: let json = use "std/json";

let parse = turn(text: Str) -> Any {
    return call("json_parse", text);
};

let stringify = turn(value: Any) -> Str {
    return call("json_stringify", value);
};

return {
    "parse": parse,
    "stringify": stringify
};
"#;

const TIME_SOURCE: &str = r#"
// Time Module
// Usage: let time = use "std/time";

let now = turn() -> Num {
    return call("time_now", null);
};

let sleep = turn(seconds: Num) -> Void {
    call("sleep", seconds);
    return null;
};

return {
    "now": now,
    "sleep": sleep
};
"#;

const REGEX_SOURCE: &str = r#"
// Regex Module
// Usage: let re = use "std/regex";

let matches = turn(pattern: Str, text: Str) -> Bool {
    return call("regex_match", { "pattern": pattern, "text": text });
};

let replace = turn(pattern: Str, text: Str, replacement: Str) -> Str {
    return call("regex_replace", {
        "pattern": pattern,
        "text": text,
        "replacement": replacement
    });
};

return {
    "matches": matches,
    "replace": replace
};
"#;
