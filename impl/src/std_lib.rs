pub fn get_module_source(path: &str) -> Option<&'static str> {
    match path {
        "std/fs" => Some(include_str!("std/fs.tn")),
        "std/net" => Some(include_str!("std/net.tn")),
        "std/env" => Some(include_str!("std/env.tn")),
        "std/math" => Some(include_str!("std/math.tn")),
        "std/json" => Some(include_str!("std/json.tn")),
        "std/time" => Some(include_str!("std/time.tn")),
        "std/regex" => Some(include_str!("std/regex.tn")),
        _ => None,
    }
}
