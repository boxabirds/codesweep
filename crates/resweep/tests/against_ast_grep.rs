//! The linked engine against the one it replaces.
//!
//! The port's definition of correct is sameness, and the installed ast-grep
//! binary is the thing it has to be the same as. Comparing against it directly
//! is stronger than comparing against numbers written down by hand, because a
//! hand-written number is only ever as good as the run that produced it.
//!
//! Skipped when ast-grep is not installed. A missing oracle is not a pass, so
//! the skip says so out loud rather than counting as one.

use std::path::{Path, PathBuf};
use std::process::Command;

use resweep::rules::Rule;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn ast_grep() -> Option<String> {
    for name in ["ast-grep", "sg"] {
        if Command::new(name).arg("--version").output().is_ok() {
            return Some(name.to_string());
        }
    }
    None
}

/// What the binary reports for one rule over one file: one-based start line
/// and matched text, in the order it emits them.
fn oracle(binary: &str, rule: &Path, file: &Path) -> Vec<(usize, String)> {
    let out = Command::new(binary)
        .arg("scan")
        .arg("--rule")
        .arg(rule)
        .arg("--json=compact")
        .arg(file)
        .output()
        .expect("ast-grep runs");
    let body = String::from_utf8_lossy(&out.stdout);
    let body = body.trim();
    if body.is_empty() {
        return vec![];
    }
    let matches: serde_json::Value = serde_json::from_str(body).expect("the oracle emits JSON");
    matches
        .as_array()
        .expect("the oracle emits an array")
        .iter()
        .map(|m| {
            let line = m["range"]["start"]["line"]
                .as_u64()
                .expect("a match carries a start line") as usize;
            let text = m["text"].as_str().expect("a match carries its text");
            // The oracle counts lines from zero. The ledger counts from one so
            // that every number it prints is the number an editor shows.
            (line + 1, text.to_string())
        })
        .collect()
}

fn compare(rule_file: &str, source_name: &str, source: &str) {
    let Some(binary) = ast_grep() else {
        eprintln!("SKIP: ast-grep is not installed, so there is no oracle to compare against");
        return;
    };
    let dir = std::env::temp_dir().join(format!("resweep-engine-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a place to put the file");
    let file = dir.join(source_name);
    std::fs::write(&file, source).expect("the file is written");

    let rule_path = repo().join(rule_file);
    let rule = Rule::load(&rule_path).expect("the shipped rule loads");
    let mine: Vec<(usize, String)> = rule
        .matches(source)
        .into_iter()
        .map(|m| (m.start_line, m.text))
        .collect();
    let theirs = oracle(&binary, &rule_path, &file);

    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(theirs, mine, "rule {rule_file} over {source_name}");
    assert!(!mine.is_empty(), "the comparison found nothing to compare");
}

#[test]
fn the_shipped_typescript_catch_rule_agrees() {
    compare(
        "rules/ts-catch-clause.yml",
        "a.ts",
        "\
export function a() { try { x(); } catch (e) { return []; } }
export function b() {
  try {
    x();
  } catch (e) {
    throw e;
  }
}
export function c() { try { x(); } catch { return null; } }
",
    );
}

#[test]
fn the_shipped_tsx_catch_rule_agrees() {
    compare(
        "rules/tsx-catch-clause.yml",
        "a.tsx",
        "\
export const C = () => {
  try { load(); } catch (e) { return <span>failed</span>; }
  return <div>{(() => { try { g(); } catch (e) { return null; } })()}</div>;
};
",
    );
}

#[test]
fn the_shipped_logical_default_rule_agrees() {
    // A pattern rule rather than a kind rule, and one that matches many times
    // on one line, so the ordering of overlapping matches is compared too.
    compare(
        "rules/ts-logical-default.yml",
        "b.ts",
        "\
const a = x || y;
const b = p ?? q;
const c = (d || e) ?? (f || g);
",
    );
}

#[test]
fn the_shipped_css_rule_agrees() {
    compare(
        "rules/css-literal-colour.yml",
        "c.css",
        "\
.a { color: #fff; }
.b { color: var(--text); }
.c { background: rgba(0, 0, 0, 0.5); }
.d { border-color: black; }
",
    );
}
