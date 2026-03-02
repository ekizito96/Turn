use std::fs;
use tempfile::TempDir;
use turn::{FileStore, Runner, ToolRegistry};

#[test]
fn test_package_import() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // 1. Create .turn_modules directory
    let modules_dir = root.join(".turn_modules");
    fs::create_dir(&modules_dir).unwrap();

    // 2. Create a package 'math_pkg'
    let pkg_path = modules_dir.join("math_pkg.tn");
    fs::write(
        &pkg_path,
        r#"
    let add = turn {
        let a = recall("a");
        let b = recall("b");
        return a + b;
    };
    return { "add": add };
"#,
    )
    .unwrap();

    // 3. Create main script that uses it
    let main_path = root.join("main.tn");
    fs::write(
        &main_path,
        r#"
let math = use "math_pkg";
let sum = call(math["add"], { "a": 10, "b": 20 });
return sum;
"#,
    )
    .unwrap();

    // 4. Run
    let store = FileStore::new(root.join(".store"));
    let tools = ToolRegistry::new();
    let mut runner = Runner::new(store, tools);

    let result = runner
        .run(
            "test_pkg",
            &fs::read_to_string(&main_path).unwrap(),
            Some(main_path.clone()),
        )
        .unwrap();

    assert_eq!(result.to_string(), "30");
}
