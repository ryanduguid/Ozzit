//! Replay generate_selftest_examples.py's recorded parsing and assertion results.
use ozzit::selftest;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;

fn corpus() -> Value {
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/selftest-contracts.json.gz"),
    )
    .unwrap();
    serde_json::from_reader(flate2::read::GzDecoder::new(&bytes[..])).unwrap()
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect()
}

fn number_differences(data: &Value, differences: &mut Vec<String>) {
    for record in data["numbers"].as_array().unwrap() {
        let text = record["text"].as_str().unwrap();
        let got = selftest::number(text).map(|(v, t)| json!([v.shown(), t.shown()]));
        if got.as_ref().unwrap_or(&Value::Null) != &record["shown"] {
            differences.push(format!("number {text:?}: {got:?} != {}", record["shown"]));
        }
    }
    assert_eq!(data["numbers"].as_array().unwrap().len(), 42);
}

fn body_differences(data: &Value, library: &BTreeSet<String>, differences: &mut Vec<String>) {
    let bodies = data["bodies"].as_array().unwrap();
    assert_eq!(bodies.len(), 600);
    for (i, record) in bodies.iter().enumerate() {
        let body = record["body"].as_str().unwrap();
        let (examples, bindings) = selftest::examples_of(body);
        let got = examples
            .iter()
            .map(|e| json!([e.formula, e.results]))
            .collect::<Vec<_>>();
        if json!(got) != record["examples"] {
            differences.push(format!("examples {i}: {got:?} != {}", record["examples"]));
            continue;
        }
        let pairs = bindings
            .iter()
            .map(|(k, v)| json!([k, v]))
            .collect::<Vec<_>>();
        if json!(pairs) != record["bindings"] {
            differences.push(format!("bindings {i}: {pairs:?} != {}", record["bindings"]));
        }
        let checks = examples
            .iter()
            .enumerate()
            .map(|(n, e)| selftest::checks_for(&format!("Fn{i}λ"), e, n + 1, library, &bindings))
            .collect::<Vec<_>>();
        if json!(checks) != record["checks"] {
            differences.push(format!("checks {i}: {checks:?} != {}", record["checks"]));
        }
        let name = format!("Fn{i}λ{}", if i % 7 == 0 { "DV" } else { "" });
        let help = selftest::help_check(&name, if i % 5 == 0 { "{1}" } else { body });
        if help != record["help"].as_str().unwrap() {
            differences.push(format!("help {i}: {help:?} != {}", record["help"]));
        }
    }
}

fn formula_differences(data: &Value, library: &BTreeSet<String>, differences: &mut Vec<String>) {
    for record in data["qualify"].as_array().unwrap() {
        let formula = record["formula"].as_str().unwrap();
        let got = selftest::qualify(formula, library);
        if got != record["qualified"].as_str().unwrap() {
            differences.push(format!("qualify {formula:?}: {got:?}"));
        }
    }
    for record in data["alone"].as_array().unwrap() {
        let formula = record["formula"].as_str().unwrap();
        let bound = strings(&record["bound"]);
        let bound = bound.iter().map(String::as_str).collect::<BTreeSet<_>>();
        let got = selftest::stands_alone(formula, library, &bound);
        if got != record["alone"].as_bool().unwrap() {
            differences.push(format!("stands_alone {formula:?} {bound:?}: {got}"));
        }
    }
}

#[test]
fn generator_functions_match_the_python_tool() {
    let data = corpus();
    let library = strings(&data["library"])
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut differences = Vec::new();
    number_differences(&data, &mut differences);

    body_differences(&data, &library, &mut differences);

    formula_differences(&data, &library, &mut differences);
    assert!(
        differences.is_empty(),
        "{} differences:\n{}",
        differences.len(),
        differences.join("\n")
    );
}

#[test]
fn the_committed_fragment_is_what_the_sources_generate() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let text = selftest::generate(&root.join("src")).unwrap();
    let committed = std::fs::read_to_string(root.join(selftest::OUTPUT)).unwrap();
    assert_eq!(text, committed.replace("\r\n", "\n"));
    assert!(text.is_ascii());
}
