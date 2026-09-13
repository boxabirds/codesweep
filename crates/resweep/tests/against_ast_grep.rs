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
    // Named per case, not per process. Tests in one binary run in parallel
    // threads, and a shared directory that each case removes at the end takes
    // another case's file with it. That produced a failure in one case and a
    // pass in the same case on the next run, which is worse than either.
    let dir = std::env::temp_dir().join(format!(
        "resweep-engine-{}-{}",
        std::process::id(),
        source_name
    ));
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

#[test]
fn the_two_logical_default_rules_agree_on_their_own_languages() {
    compare(
        "rules/tsx-logical-default.yml",
        "d.tsx",
        "export const C = ({a, b}) => <div>{a || b}</div>;\nconst x = p ?? q;\n",
    );
}

#[test]
fn a_rule_that_matches_nothing_agrees_that_it_matched_nothing() {
    // The empty case is worth comparing too. A matcher that silently returns
    // nothing looks identical to one that correctly finds nothing, and the
    // difference only shows when the same input produces sites for the oracle.
    let Some(binary) = ast_grep() else {
        eprintln!("SKIP: ast-grep is not installed, so there is no oracle to compare against");
        return;
    };
    let dir = std::env::temp_dir().join(format!("resweep-empty-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a place to put the file");
    let file = dir.join("e.ts");
    let source = "export function a() { return 1; }\n";
    std::fs::write(&file, source).expect("the file is written");
    let rule_path = repo().join("rules/ts-catch-clause.yml");
    let rule = resweep::rules::Rule::load(&rule_path).expect("the shipped rule loads");
    assert!(rule.matches(source).is_empty());
    assert!(oracle(&binary, &rule_path, &file).is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

/// Every shipped rule over a whole real repository, compared against the
/// installed matching engine file by file.
///
/// Not against a recorded count. The monorepo changes daily, so a frozen
/// number would fail for the wrong reason within a day and get deleted, which
/// is how a suite stops meaning anything. The oracle is the engine itself, and
/// it is asked the same question over the same files at the same moment.
fn compare_over_repository(label: &str, root: &Path) {
    let Some(binary) = ast_grep() else {
        eprintln!("SKIP: ast-grep is not installed, so there is no oracle to compare against");
        return;
    };
    let rules_dir = repo().join("rules");
    let mut rules: Vec<PathBuf> = std::fs::read_dir(&rules_dir)
        .expect("the rules directory exists")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("yml"))
        .collect();
    rules.sort();
    assert!(rules.len() >= 5, "only {} rules found", rules.len());

    let files = resweep::discovery::discover(root).files;
    let mut compared = 0usize;
    let mut total = 0usize;

    for rule_path in &rules {
        let rule = resweep::rules::Rule::load(rule_path).expect("the shipped rule loads");
        let extensions = resweep::languages::extensions_for(&rule.language).unwrap_or(&[]);
        for relname in &files {
            let ext = Path::new(relname)
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();
            if !extensions.contains(&ext.as_str()) {
                continue;
            }
            let full = root.join(relname);
            let Ok(bytes) = std::fs::read(&full) else { continue };
            // Skipped rather than compared: the oracle and the engine may
            // disagree about how to repair invalid bytes, and that is a
            // question about lossy decoding, not about matching.
            let Ok(source) = String::from_utf8(bytes) else { continue };

            let mine: Vec<(usize, String)> = rule
                .matches(&source)
                .into_iter()
                .map(|m| (m.start_line, m.text))
                .collect();
            let theirs = oracle(&binary, rule_path, &full);
            assert_eq!(
                theirs,
                mine,
                "{label}: {} over {relname}",
                rule_path.file_name().unwrap_or_default().to_string_lossy()
            );
            compared += 1;
            total += mine.len();
        }
    }

    // A comparison that examined nothing is not evidence of agreement, and
    // neither is one that examined files and found nothing in them: a matcher
    // returning nothing agrees perfectly with an oracle returning nothing.
    // Both counts have to be non-zero for this to have said anything.
    assert!(compared > 0, "{label}: no file matched any rule's language");
    assert!(total > 0, "{label}: {compared} files compared and not one site found in any of them");
    eprintln!("{label}: {compared} file-and-rule pairs compared, {total} sites agreed");
}

// Only the monorepo. The shipped rules are TypeScript, tsx and CSS, and
// neither this repository nor the pinned fixture holds enough of those for the
// comparison to say anything: this one is Rust and shell, and the fixture is
// JavaScript. Both were tried, and the guard above caught them producing a
// green result over nothing, which is the failure this whole project exists to
// remove and is not one to ship in its own suite.

#[test]
fn every_rule_over_the_real_monorepo() {
    let repo_path = std::env::var("CEETRIX_REPO").unwrap_or_else(|_| {
        format!("{}/expts/claude-backlog", std::env::var("HOME").unwrap_or_default())
    });
    let path = PathBuf::from(&repo_path);
    if !path.join(".git").exists() {
        eprintln!("SKIP: no monorepo checkout at {repo_path}");
        return;
    }
    compare_over_repository("the monorepo", &path);
}

