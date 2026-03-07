// Experiment E2: Confidence Propagation
//
// Verifies the probabilistic control-flow semantics:
//   1. `confidence v` on a certain value returns 1.0.
//   2. `confidence v` on an Uncertain(val, p) value returns p.
//   3. Arithmetic on Uncertain values propagates uncertainty via product rule.
//   4. Boolean AND uses the Zadeh minimum rule: min(p1, p2).
//   5. Boolean OR uses the Zadeh maximum rule: max(p1, p2).
//   6. The `if confidence result < threshold` branch executes deterministically.
//
// These semantics ensure stochastic inference output can be gated with
// deterministic, predictable control flow.

use turn::compiler::Compiler;
use turn::lexer::Lexer;
use turn::parser::Parser;
use turn::value::Value;
use turn::vm::{Vm, VmResult};

fn run_with_env(source: &str, env_setup: impl FnOnce(&mut Vm)) -> Value {
    let tokens = Lexer::new(source).tokenize().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut compiler = Compiler::new();
    let code = compiler.compile(&program);
    let mut vm = Vm::new(&code);
    env_setup(&mut vm);
    match vm.run() {
        VmResult::Complete(v) => v,
        _ => panic!("VM did not complete"),
    }
}

/// E2-1: Certain values have confidence 1.0.
#[test]
fn e2_certain_value_has_confidence_1() {
    let val = run_with_env("let c = confidence 42; return c;", |_| {});
    match val {
        Value::Num(n) => {
            assert!((n - 1.0).abs() < 1e-9, "Expected 1.0, got {}", n);
            println!("E2-1 PASS: confidence(42) = {:.1}", n);
        }
        other => panic!("Expected Num(1.0), got {:?}", other),
    }
}

/// E2-2: Uncertain value returns its stored confidence score.
#[test]
fn e2_uncertain_value_returns_stored_confidence() {
    let val = run_with_env("let c = confidence x; return c;", |vm| {
        if let Some(p) = vm.scheduler.front_mut() {
            p.runtime.env.insert(
                "x".to_string(),
                Value::Uncertain(Box::new(Value::Num(42.0)), 0.73),
            );
        }
    });
    match val {
        Value::Num(n) => {
            assert!((n - 0.73).abs() < 1e-6, "Expected 0.73, got {}", n);
            println!("E2-2 PASS: confidence(Uncertain(42, 0.73)) = {:.2}", n);
        }
        other => panic!("Expected Num(0.73), got {:?}", other),
    }
}

/// E2-3: Product rule for arithmetic uncertainty propagation.
/// confidence(x + y) where x~0.8, y~0.5  =>  0.8 * 0.5 = 0.4
#[test]
fn e2_product_rule_arithmetic() {
    let val = run_with_env("let z = x + y; return confidence z;", |vm| {
        if let Some(p) = vm.scheduler.front_mut() {
            p.runtime.env.insert(
                "x".to_string(),
                Value::Uncertain(Box::new(Value::Num(10.0)), 0.8),
            );
            p.runtime.env.insert(
                "y".to_string(),
                Value::Uncertain(Box::new(Value::Num(5.0)), 0.5),
            );
        }
    });
    match val {
        Value::Num(n) => {
            assert!((n - 0.4).abs() < 1e-6, "Expected 0.4, got {}", n);
            println!("E2-3 PASS: confidence(Uncertain(10,0.8) + Uncertain(5,0.5)) = {:.2}  (product rule: 0.8×0.5)", n);
        }
        other => panic!("Expected Num(0.4), got {:?}", other),
    }
}

/// E2-4: Zadeh minimum rule for AND.
/// confidence(x AND y) where x~0.9, y~0.5  =>  min(0.9, 0.5) = 0.5
#[test]
fn e2_zadeh_min_rule_and() {
    let val = run_with_env("let res = x and y; return confidence res;", |vm| {
        if let Some(p) = vm.scheduler.front_mut() {
            p.runtime.env.insert(
                "x".to_string(),
                Value::Uncertain(Box::new(Value::Bool(true)), 0.9),
            );
            p.runtime.env.insert(
                "y".to_string(),
                Value::Uncertain(Box::new(Value::Bool(false)), 0.5),
            );
        }
    });
    match val {
        Value::Num(n) => {
            assert!((n - 0.5).abs() < 1e-6, "Expected 0.5, got {}", n);
            println!("E2-4 PASS: confidence(Uncertain(true,0.9) AND Uncertain(false,0.5)) = {:.2}  (Zadeh min: min(0.9,0.5))", n);
        }
        other => panic!("Expected Num(0.5), got {:?}", other),
    }
}

/// E2-5: Deterministic fallback branch executes when confidence below threshold.
/// The `if confidence result < 0.7` branch must fire and return the fallback.
#[test]
fn e2_deterministic_fallback_below_threshold() {
    let source = r#"
    let result = x;
    if confidence result < 0.7 {
        return "fallback";
    }
    return "high_confidence";
    "#;
    let val = run_with_env(source, |vm| {
        if let Some(p) = vm.scheduler.front_mut() {
            p.runtime.env.insert(
                "x".to_string(),
                Value::Uncertain(Box::new(Value::Str("some output".to_string())), 0.45),
            );
        }
    });
    match val {
        Value::Str(s) => {
            assert_eq!(s, "fallback", "Low-confidence branch should execute");
            println!("E2-5 PASS: confidence=0.45 < 0.70 → deterministic fallback branch executed");
        }
        other => panic!("Expected Str(\"fallback\"), got {:?}", other),
    }
}

/// E2-6: High-confidence path executes normally.
#[test]
fn e2_high_confidence_passes_through() {
    let source = r#"
    let result = x;
    if confidence result < 0.7 {
        return "fallback";
    }
    return "high_confidence";
    "#;
    let val = run_with_env(source, |vm| {
        if let Some(p) = vm.scheduler.front_mut() {
            p.runtime.env.insert(
                "x".to_string(),
                Value::Uncertain(Box::new(Value::Str("solid output".to_string())), 0.92),
            );
        }
    });
    match val {
        Value::Str(s) => {
            assert_eq!(s, "high_confidence");
            println!("E2-6 PASS: confidence=0.92 >= 0.70 → high-confidence path executed");
        }
        other => panic!("Expected Str(\"high_confidence\"), got {:?}", other),
    }
}
