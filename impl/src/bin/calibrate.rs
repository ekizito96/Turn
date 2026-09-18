//! Calibration harness for Turn's confidence measurement.
//!
//! Answers one question: does the per-field confidence produced by `infer`
//! predict whether that field is actually correct? Propagating a number that
//! carries no signal would make the whole `Uncertain` branch of the language
//! elegant machinery on noise, so this measures it before more is built on top.
//!
//! Dataset is JSONL, one case per line:
//!   {"prompt": "...", "expected": {"field": <scalar>, ...}}
//!
//! Requires a provider that reports log probabilities and a built driver at
//! `.turn_modules/openai_provider.wasm`.

use anyhow::{bail, Context, Result};
use clap::Parser;
use serde_json::{json, Value as Json};
use std::path::PathBuf;
use turn::tools::ToolRegistry;
use turn::value::Value;

#[derive(Parser)]
#[command(name = "calibrate")]
#[command(about = "Measure whether Turn's inference confidence predicts correctness")]
struct Cli {
    /// JSONL file of {"prompt": ..., "expected": {...}} cases
    #[arg(long)]
    dataset: PathBuf,

    /// Maximum cases to run. Each case is one paid API call.
    #[arg(long, default_value_t = 50)]
    limit: usize,

    /// Print the generated Turn program for the first case and exit without calling any API
    #[arg(long)]
    dry_run: bool,

    /// Write the full report as JSON
    #[arg(long)]
    out: Option<PathBuf>,

    /// Number of reliability bins
    #[arg(long, default_value_t = 10)]
    bins: usize,
}

struct Case {
    prompt: String,
    expected: serde_json::Map<String, Json>,
}

/// One measured field of one case.
struct Observation {
    field: String,
    confidence: Option<f64>,
    correct: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let cases = load_cases(&cli.dataset, cli.limit)?;
    if cases.is_empty() {
        bail!("no cases in {}", cli.dataset.display());
    }
    let schema = infer_schema(&cases);
    if schema.is_empty() {
        bail!("no fields found in any `expected` object");
    }

    if cli.dry_run {
        println!("{}", turn_program(&schema, &cases[0].prompt));
        return Ok(());
    }

    let tools = ToolRegistry::new();
    let mut observations = Vec::new();
    let mut failures = Vec::new();

    for (i, case) in cases.iter().enumerate() {
        let source = turn_program(&schema, &case.prompt);
        match turn::run_with_tools(&source, &tools) {
            Ok(result) => observations.extend(observe(&result, &case.expected)),
            Err(e) => failures.push((i, e.to_string())),
        }
    }

    let report = build_report(&observations, &failures, cli.bins);
    print_report(&report, &observations, cli.bins);

    if let Some(path) = cli.out {
        std::fs::write(&path, serde_json::to_string_pretty(&report)?)
            .with_context(|| format!("writing {}", path.display()))?;
        println!("\nreport written to {}", path.display());
    }

    Ok(())
}

fn load_cases(path: &PathBuf, limit: usize) -> Result<Vec<Case>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut cases = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if cases.len() >= limit {
            break;
        }
        let parsed: Json = serde_json::from_str(line)
            .with_context(|| format!("{}:{} is not valid JSON", path.display(), n + 1))?;
        let prompt = parsed
            .get("prompt")
            .and_then(|p| p.as_str())
            .with_context(|| format!("{}:{} has no string `prompt`", path.display(), n + 1))?
            .to_string();
        let expected = parsed
            .get("expected")
            .and_then(|e| e.as_object())
            .with_context(|| format!("{}:{} has no object `expected`", path.display(), n + 1))?
            .clone();
        cases.push(Case { prompt, expected });
    }
    Ok(cases)
}

/// Field order is first-seen across the dataset so the generated struct is stable.
fn infer_schema(cases: &[Case]) -> Vec<(String, &'static str)> {
    let mut schema: Vec<(String, &'static str)> = Vec::new();
    for case in cases {
        for (key, value) in &case.expected {
            if schema.iter().any(|(k, _)| k == key) || value.is_null() {
                continue;
            }
            let ty = if value.is_boolean() {
                "Bool"
            } else if value.is_number() {
                "Num"
            } else {
                "Str"
            };
            schema.push((key.clone(), ty));
        }
    }
    schema
}

fn turn_program(schema: &[(String, &'static str)], prompt: &str) -> String {
    let fields = schema
        .iter()
        .map(|(name, ty)| format!("    {}: {}", name, ty))
        .collect::<Vec<_>>()
        .join(",\n");
    format!(
        "struct Extracted {{\n{}\n}};\n\nlet result = infer Extracted {{\n    {};\n}};\n\nreturn result;\n",
        fields,
        turn_string_literal(prompt)
    )
}

/// The Turn lexer rejects any escape outside this set, so nothing else may be escaped.
fn turn_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

fn observe(result: &Value, expected: &serde_json::Map<String, Json>) -> Vec<Observation> {
    let inner = match result {
        Value::Uncertain(v, _) => v.as_ref(),
        other => other,
    };
    let fields = match inner {
        Value::Struct(_, fields) => fields,
        Value::Map(fields) => fields,
        _ => return Vec::new(),
    };

    expected
        .iter()
        .map(|(field, want)| {
            let got = fields.get(field);
            let (confidence, got_value) = match got {
                Some(Value::Uncertain(v, p)) => (Some(*p), Some(v.as_ref())),
                Some(v) => (None, Some(v)),
                None => (None, None),
            };
            Observation {
                field: field.clone(),
                confidence,
                correct: got_value.is_some_and(|v| matches_expected(v, want)),
            }
        })
        .collect()
}

fn matches_expected(got: &Value, want: &Json) -> bool {
    match want {
        Json::Bool(b) => matches!(got, Value::Bool(g) if g == b),
        Json::Number(n) => {
            let want = match n.as_f64() {
                Some(w) => w,
                None => return false,
            };
            let got = match got {
                Value::Num(g) => *g,
                Value::Str(s) => match s.trim().parse::<f64>() {
                    Ok(g) => g,
                    Err(_) => return false,
                },
                _ => return false,
            };
            (got - want).abs() <= 1e-6 * want.abs().max(1.0)
        }
        Json::String(s) => match got {
            Value::Str(g) => normalise(g) == normalise(s),
            other => normalise(&other.to_string()) == normalise(s),
        },
        Json::Null => matches!(got, Value::Null),
        _ => false,
    }
}

/// String extraction is graded on case and whitespace insensitive equality.
fn normalise(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn build_report(observations: &[Observation], failures: &[(usize, String)], bins: usize) -> Json {
    let measured: Vec<(f64, bool)> = observations
        .iter()
        .filter_map(|o| o.confidence.map(|c| (c, o.correct)))
        .collect();

    let unmeasured = observations.len() - measured.len();
    let n = measured.len();

    if n == 0 {
        return json!({
            "observations": observations.len(),
            "measured": 0,
            "unmeasured": unmeasured,
            "case_failures": failures.len(),
            "verdict": "no confidence was measured; check the provider supports logprobs",
        });
    }

    let correct = measured.iter().filter(|(_, c)| *c).count();
    let accuracy = correct as f64 / n as f64;
    let mean_confidence = measured.iter().map(|(c, _)| c).sum::<f64>() / n as f64;
    let brier = measured
        .iter()
        .map(|(c, ok)| {
            let y = if *ok { 1.0 } else { 0.0 };
            (c - y).powi(2)
        })
        .sum::<f64>()
        / n as f64;

    let reliability = reliability_bins(&measured, bins);
    let ece = ece(&measured, bins);
    let auroc = auroc(&measured);
    let correlation = point_biserial(&measured);

    json!({
        "observations": observations.len(),
        "measured": n,
        "unmeasured": unmeasured,
        "case_failures": failures.len(),
        "failures": failures.iter().map(|(i, e)| json!({"case": i, "error": e})).collect::<Vec<_>>(),
        "accuracy": accuracy,
        "mean_confidence": mean_confidence,
        "overconfidence": mean_confidence - accuracy,
        "brier": brier,
        "ece": ece,
        "auroc": auroc,
        "point_biserial_r": correlation,
        "reliability": reliability.iter().map(|b| json!({
            "lower": b.lower,
            "upper": b.upper,
            "count": b.count,
            "mean_confidence": b.mean_confidence,
            "accuracy": b.accuracy,
        })).collect::<Vec<_>>(),
        "verdict": verdict(n, auroc, ece),
    })
}

struct Bin {
    lower: f64,
    upper: f64,
    count: usize,
    mean_confidence: f64,
    accuracy: f64,
}

fn reliability_bins(measured: &[(f64, bool)], bins: usize) -> Vec<Bin> {
    let bins = bins.max(1);
    let width = 1.0 / bins as f64;
    (0..bins)
        .map(|i| {
            let lower = i as f64 * width;
            let upper = if i == bins - 1 { 1.0 } else { lower + width };
            let members: Vec<&(f64, bool)> = measured
                .iter()
                .filter(|(c, _)| *c >= lower && (*c < upper || (i == bins - 1 && *c <= upper)))
                .collect();
            let count = members.len();
            let (mean_confidence, accuracy) = if count == 0 {
                (0.0, 0.0)
            } else {
                (
                    members.iter().map(|(c, _)| c).sum::<f64>() / count as f64,
                    members.iter().filter(|(_, ok)| *ok).count() as f64 / count as f64,
                )
            };
            Bin {
                lower,
                upper,
                count,
                mean_confidence,
                accuracy,
            }
        })
        .collect()
}

/// Expected calibration error: how far the stated probabilities sit from the
/// observed frequencies, averaged over bins and weighted by bin population.
fn ece(measured: &[(f64, bool)], bins: usize) -> f64 {
    let n = measured.len() as f64;
    if n == 0.0 {
        return 0.0;
    }
    reliability_bins(measured, bins)
        .iter()
        .map(|b| (b.count as f64 / n) * (b.accuracy - b.mean_confidence).abs())
        .sum()
}

/// Probability that a randomly chosen correct field scores above a randomly
/// chosen incorrect one. 0.5 means the confidence carries no ranking signal.
fn auroc(measured: &[(f64, bool)]) -> Option<f64> {
    let positives: Vec<f64> = measured
        .iter()
        .filter(|(_, ok)| *ok)
        .map(|(c, _)| *c)
        .collect();
    let negatives: Vec<f64> = measured
        .iter()
        .filter(|(_, ok)| !*ok)
        .map(|(c, _)| *c)
        .collect();
    if positives.is_empty() || negatives.is_empty() {
        return None;
    }
    let mut wins = 0.0;
    for p in &positives {
        for n in &negatives {
            if p > n {
                wins += 1.0;
            } else if (p - n).abs() < f64::EPSILON {
                wins += 0.5;
            }
        }
    }
    Some(wins / (positives.len() * negatives.len()) as f64)
}

fn point_biserial(measured: &[(f64, bool)]) -> Option<f64> {
    let n = measured.len() as f64;
    if n < 2.0 {
        return None;
    }
    let xs: Vec<f64> = measured.iter().map(|(c, _)| *c).collect();
    let ys: Vec<f64> = measured
        .iter()
        .map(|(_, ok)| if *ok { 1.0 } else { 0.0 })
        .collect();
    let mx = xs.iter().sum::<f64>() / n;
    let my = ys.iter().sum::<f64>() / n;
    let cov: f64 = xs
        .iter()
        .zip(&ys)
        .map(|(x, y)| (x - mx) * (y - my))
        .sum::<f64>();
    let sx: f64 = xs.iter().map(|x| (x - mx).powi(2)).sum::<f64>().sqrt();
    let sy: f64 = ys.iter().map(|y| (y - my).powi(2)).sum::<f64>().sqrt();
    if sx == 0.0 || sy == 0.0 {
        return None;
    }
    Some(cov / (sx * sy))
}

/// Separates "wrong scale" from "no signal". The first is repairable with a
/// calibration map; the second means the measurement is not worth propagating.
fn verdict(n: usize, auroc: Option<f64>, ece: f64) -> String {
    if n < 100 {
        return format!("only {} measured fields; too few to conclude anything", n);
    }
    match auroc {
        None => "every field was correct or every field was wrong; no discrimination measurable"
            .to_string(),
        Some(a) if a < 0.55 => {
            "confidence does not rank correct fields above incorrect ones. The signal is unusable \
             and propagating it is not justified."
                .to_string()
        }
        Some(a) if ece > 0.10 => format!(
            "confidence ranks correctly (AUROC {:.3}) but the absolute values are wrong \
             (ECE {:.3}). Usable for comparison, not as a probability, until a calibration map \
             is fitted.",
            a, ece
        ),
        Some(a) => format!(
            "confidence both ranks (AUROC {:.3}) and is calibrated (ECE {:.3}). Propagation is \
             justified.",
            a, ece
        ),
    }
}

fn print_report(report: &Json, observations: &[Observation], bins: usize) {
    println!("\n=== calibration ===");
    for key in [
        "observations",
        "measured",
        "unmeasured",
        "case_failures",
        "accuracy",
        "mean_confidence",
        "overconfidence",
        "brier",
        "ece",
        "auroc",
        "point_biserial_r",
    ] {
        if let Some(v) = report.get(key) {
            println!("{:<18} {}", key, v);
        }
    }

    if report.get("measured").and_then(|m| m.as_u64()).unwrap_or(0) > 0 {
        println!("\n=== reliability ({} bins) ===", bins);
        println!(
            "{:<14} {:>6} {:>10} {:>10}",
            "range", "n", "mean conf", "accuracy"
        );
        if let Some(rows) = report.get("reliability").and_then(|r| r.as_array()) {
            for row in rows {
                let count = row["count"].as_u64().unwrap_or(0);
                if count == 0 {
                    continue;
                }
                println!(
                    "{:<14} {:>6} {:>10.3} {:>10.3}",
                    format!(
                        "[{:.2}, {:.2})",
                        row["lower"].as_f64().unwrap_or(0.0),
                        row["upper"].as_f64().unwrap_or(0.0)
                    ),
                    count,
                    row["mean_confidence"].as_f64().unwrap_or(0.0),
                    row["accuracy"].as_f64().unwrap_or(0.0),
                );
            }
        }

        println!("\n=== per field ===");
        let mut names: Vec<&str> = observations.iter().map(|o| o.field.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        println!(
            "{:<20} {:>6} {:>10} {:>10}",
            "field", "n", "mean conf", "accuracy"
        );
        for name in names {
            let rows: Vec<&Observation> = observations
                .iter()
                .filter(|o| o.field == name && o.confidence.is_some())
                .collect();
            if rows.is_empty() {
                continue;
            }
            let mean = rows.iter().filter_map(|o| o.confidence).sum::<f64>() / rows.len() as f64;
            let acc = rows.iter().filter(|o| o.correct).count() as f64 / rows.len() as f64;
            println!(
                "{:<20} {:>6} {:>10.3} {:>10.3}",
                name,
                rows.len(),
                mean,
                acc
            );
        }
    }

    if let Some(failures) = report.get("failures").and_then(|f| f.as_array()) {
        if !failures.is_empty() {
            println!("\n=== failed cases ===");
            for failure in failures.iter().take(10) {
                println!("case {}: {}", failure["case"], failure["error"]);
            }
        }
    }

    println!("\n{}", report["verdict"].as_str().unwrap_or(""));
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;

    #[test]
    fn string_literals_escape_only_what_the_lexer_accepts() {
        assert_eq!(turn_string_literal(r#"say "hi""#), r#""say \"hi\"""#);
        assert_eq!(turn_string_literal("a\\b"), r#""a\\b""#);
        assert_eq!(turn_string_literal("one\ntwo"), r#""one\ntwo""#);
        assert_eq!(turn_string_literal("caf\u{e9} 50%"), "\"caf\u{e9} 50%\"");
    }

    #[test]
    fn schema_types_come_from_the_expected_values() {
        let cases = vec![Case {
            prompt: "x".into(),
            expected: serde_json::from_str(r#"{"a":"s","b":1,"c":true,"d":null}"#).unwrap(),
        }];
        let schema = infer_schema(&cases);
        assert_eq!(
            schema,
            vec![
                ("a".to_string(), "Str"),
                ("b".to_string(), "Num"),
                ("c".to_string(), "Bool"),
            ]
        );
    }

    #[test]
    fn observe_reads_per_field_confidence() {
        let mut fields = IndexMap::new();
        fields.insert(
            "company".to_string(),
            Value::Uncertain(Box::new(Value::Str("Acme Corp".into())), 0.99),
        );
        fields.insert(
            "revenue".to_string(),
            Value::Uncertain(Box::new(Value::Num(41.0)), 0.4),
        );
        let result = Value::Uncertain(Box::new(Value::Struct("Extracted".into(), fields)), 0.7);
        let expected = serde_json::from_str(r#"{"company":"acme   corp","revenue":42}"#).unwrap();

        let obs = observe(&result, &expected);
        let company = obs.iter().find(|o| o.field == "company").unwrap();
        let revenue = obs.iter().find(|o| o.field == "revenue").unwrap();

        assert_eq!(company.confidence, Some(0.99));
        assert!(company.correct, "whitespace and case are normalised");
        assert_eq!(revenue.confidence, Some(0.4));
        assert!(!revenue.correct, "41 is not 42");
    }

    #[test]
    fn missing_confidence_stays_missing() {
        let mut fields = IndexMap::new();
        fields.insert("a".to_string(), Value::Str("x".into()));
        let result = Value::Struct("Extracted".into(), fields);
        let expected = serde_json::from_str(r#"{"a":"x"}"#).unwrap();

        let obs = observe(&result, &expected);
        assert_eq!(obs[0].confidence, None);
        assert!(obs[0].correct);
    }

    #[test]
    fn auroc_detects_separation_and_its_absence() {
        assert_eq!(auroc(&[(0.9, true), (0.1, false)]), Some(1.0));
        assert_eq!(auroc(&[(0.1, true), (0.9, false)]), Some(0.0));
        assert_eq!(auroc(&[(0.5, true), (0.5, false)]), Some(0.5));
        assert_eq!(auroc(&[(0.5, true), (0.9, true)]), None);
    }

    #[test]
    fn ece_is_zero_when_stated_matches_observed() {
        let mut measured = vec![(0.9, true); 9];
        measured.push((0.9, false));
        assert!(ece(&measured, 10) < 1e-9);
    }

    #[test]
    fn ece_catches_overconfidence() {
        let mut measured = vec![(1.0, true); 5];
        measured.extend(vec![(1.0, false); 5]);
        assert!((ece(&measured, 10) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn verdict_refuses_to_conclude_from_a_small_sample() {
        assert!(verdict(20, Some(0.9), 0.01).contains("too few"));
    }

    #[test]
    fn verdict_separates_bad_scale_from_no_signal() {
        assert!(verdict(500, Some(0.51), 0.01).contains("unusable"));
        assert!(verdict(500, Some(0.80), 0.30).contains("ranks correctly"));
        assert!(verdict(500, Some(0.80), 0.02).contains("justified"));
    }
}
