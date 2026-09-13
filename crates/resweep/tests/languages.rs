//! The menu of languages, checked in both directions, and what happens to
//! source the tool cannot read.
//!
//! The defect being guarded against is a language on the menu that returns
//! nothing: a rule naming it loads, runs, finds no sites, and the sweep reports
//! full coverage of a scope it never read.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use resweep::{languages, ledger, rules};

fn binary() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/release/resweep")
}

struct Project {
    dir: PathBuf,
    session: String,
}

impl Project {
    /// A shape a real project has: source a rule covers, source a rule could
    /// cover but none was supplied for, and source no rule can ever cover.
    fn new(label: &str) -> Self {
        let session = format!("languages-{label}-{}", std::process::id());
        let dir = std::env::temp_dir().join(format!("resweep-{session}"));
        std::fs::create_dir_all(dir.join("src")).expect("a project directory");
        std::fs::write(
            dir.join("src/handler.ts"),
            "export function a() { try { x(); } catch (e) { return []; } }\n",
        )
        .expect("typescript");
        std::fs::write(
            dir.join("src/Button.tsx"),
            "export const B = () => <button>go</button>;\n",
        )
        .expect("tsx");
        std::fs::write(dir.join("src/reset.css"), ".a { color: #fff; }\n").expect("css");
        std::fs::write(dir.join("src/theme.scss"), "$brand: #fff;\n.a { color: $brand; }\n")
            .expect("scss");
        std::fs::write(dir.join("src/mixins.scss"), ".b { color: #000; }\n").expect("scss");
        std::fs::write(
            dir.join("catch.yml"),
            "id: ts-catch\nlanguage: typescript\nrule:\n  kind: catch_clause\n",
        )
        .expect("a rule");
        Self { dir, session }
    }

    fn rule(&self, name: &str, body: &str) -> String {
        let path = self.dir.join(name);
        std::fs::write(&path, body).expect("a rule file");
        path.to_string_lossy().to_string()
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(binary())
            .args(args)
            .arg("--root")
            .arg(&self.dir)
            .env(ledger::SESSION_ENV_OVERRIDE, &self.session)
            .output()
            .expect("the tool runs")
    }

    fn census(&self, sweep: &str, rule: &str) -> Output {
        self.run(&["census", sweep, "--rule", rule, "--scope", "src", "--question", "q"])
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
        std::fs::remove_dir_all(std::env::temp_dir().join(ledger::TEMP_NAMESPACE).join(&self.session)).ok();
    }
}

fn json(out: &Output) -> serde_json::Value {
    serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).expect("JSON")
}

#[test]
fn every_language_on_the_menu_returns_results_on_a_file_of_its_own_kind() {
    // The defect this story exists for. A language that loads but matches
    // nothing produces a small, clean, confident, wrong report. Each language
    // gets a snippet in its own syntax and a rule that must match it.
    let cases: &[(&str, &str, &str)] = &[
        ("typescript", "function a() { try { x(); } catch (e) {} }\n", "catch_clause"),
        ("tsx", "const A = () => { try { x(); } catch (e) {} return <b/>; };\n", "catch_clause"),
        ("javascript", "function a() { try { x(); } catch (e) {} }\n", "catch_clause"),
        ("html", "<div class=\"a\">hi</div>\n", "element"),
        ("css", ".a { color: red; }\n", "declaration"),
        ("python", "try:\n    x()\nexcept Exception:\n    pass\n", "except_clause"),
        ("rust", "fn a() { let b = 1; }\n", "let_declaration"),
        ("go", "package m\nfunc a() { b := 1; _ = b }\n", "short_var_declaration"),
        ("java", "class A { void b() { int c = 1; } }\n", "local_variable_declaration"),
        ("kotlin", "fun a() { val b = 1 }\n", "property_declaration"),
        ("c", "int a(void) { int b = 1; return b; }\n", "declaration"),
        ("cpp", "int a() { int b = 1; return b; }\n", "declaration"),
        ("csharp", "class A { void B() { int c = 1; } }\n", "local_declaration_statement"),
        ("ruby", "begin\n  x\nrescue => e\nend\n", "rescue"),
        ("php", "<?php try { x(); } catch (E $e) {} ?>\n", "catch_clause"),
        ("swift", "func a() { let b = 1 }\n", "property_declaration"),
        ("scala", "object A { val b = 1 }\n", "val_definition"),
        ("lua", "local a = 1\n", "variable_declaration"),
        ("bash", "a=1\n", "variable_assignment"),
        ("elixir", "defmodule A do\nend\n", "call"),
        ("haskell", "main = return ()\n", "bind"),
        ("json", "{\"a\": 1}\n", "pair"),
        ("yaml", "a: 1\n", "block_mapping_pair"),
    ];

    assert_eq!(
        cases.len(),
        rules::SUPPORTED_LANGUAGES.len(),
        "the menu changed and this table did not"
    );

    for (language, source, kind) in cases {
        assert!(
            rules::SUPPORTED_LANGUAGES.contains(language),
            "{language} is in this table but not on the menu"
        );
        let yaml = format!("id: probe\nlanguage: {language}\nrule:\n  kind: {kind}\n");
        let rule = rules::Rule::from_source(Path::new("probe.yml"), yaml)
            .unwrap_or_else(|e| panic!("{language}: the rule would not load: {e}"));
        let found = rule.matches(source);
        assert!(
            !found.is_empty(),
            "{language} is on the menu and returned nothing for a {kind} in its own syntax"
        );
    }
}

#[test]
fn a_language_the_build_cannot_parse_is_named_and_the_menu_is_shown() {
    let p = Project::new("named");
    let rule = p.rule("scss.yml", "id: s\nlanguage: scss\nrule:\n  kind: declaration\n");
    let out = p.census("s", &rule);
    assert!(!out.status.success());
    let message = String::from_utf8_lossy(&out.stderr);
    assert!(message.contains("scss"), "{message}");
    assert!(message.contains("cannot parse"), "{message}");
    assert!(message.contains("Supported:"), "{message}");
    assert!(message.contains("typescript"), "the supported set was not listed: {message}");
    assert!(String::from_utf8_lossy(&out.stdout).trim().is_empty());
}

#[test]
fn a_valid_language_with_a_broken_rule_is_not_blamed_on_the_language() {
    // The misclassification this replaces. Reporting a language problem for a
    // typo in the rule body sends the author to fix the wrong line.
    let p = Project::new("broken");
    let rule = p.rule("bad.yml", "id: b\nlanguage: typescript\nrule:\n  nonsense: [\n");
    let out = p.census("b", &rule);
    assert!(!out.status.success());
    let message = String::from_utf8_lossy(&out.stderr);
    assert!(message.contains("bad.yml"), "{message}");
    assert!(!message.contains("Supported:"), "a rule body problem was blamed on the language: {message}");
}

#[test]
fn source_the_build_cannot_read_is_reported_separately_and_never_silently() {
    // Both halves matter. Silence would be worse than wrong advice, and wrong
    // advice is what the separate warning removes.
    let p = Project::new("split");
    let rule = p.dir.join("catch.yml").to_string_lossy().to_string();
    let payload = json(&p.census("s", &rule));

    let coverable = payload["WARNING_uncovered_extensions"]
        .as_object()
        .expect("a warning about extensions no rule covered");
    assert!(coverable.contains_key(".css"), "css is coverable and was not offered as such");
    assert!(coverable.contains_key(".tsx"), "tsx is coverable and was not offered as such");
    assert!(!coverable.contains_key(".scss"), "scss was offered as though a rule would fix it");

    let unparseable = payload["WARNING_unparseable_extensions"]
        .as_object()
        .expect("a warning about extensions no rule can ever cover");
    assert_eq!(unparseable[".scss"], 2, "both stylesheets should be counted");
    let advice = payload["WARNING_unparseable"].as_str().expect("advice");
    assert!(advice.contains("scss"), "{advice}");
    assert!(advice.contains("by hand"), "the advice offers no way forward: {advice}");
}

#[test]
fn both_kinds_of_gap_count_against_completeness() {
    // The separation is about advice, not about arithmetic. A file the tool
    // cannot read is not a file that has been checked.
    let p = Project::new("arithmetic");
    let rule = p.dir.join("catch.yml").to_string_lossy().to_string();
    p.census("s", &rule);
    let status = json(&p.run(&["status", "s"]));
    let uncovered = status["uncovered_extensions"].as_object().expect("the recorded gaps");
    assert!(uncovered.contains_key(".scss"), "scss vanished from the record");
    assert!(uncovered.contains_key(".css"), "css vanished from the record");
    assert_eq!(status["complete"], false);
    let reasons: Vec<String> = status["incomplete_because"]
        .as_array()
        .expect("reasons")
        .iter()
        .map(|r| r.as_str().unwrap_or_default().to_string())
        .collect();
    let reach = reasons
        .iter()
        .find(|r| r.contains("no rule can reach"))
        .unwrap_or_else(|| panic!("no reason mentioned unreachable source: {reasons:?}"));
    assert!(reach.contains(".scss"), "{reach}");
    assert!(reach.contains(".css"), "{reach}");
}

#[test]
fn the_common_case_produces_no_warning_at_all() {
    // The case this design is most concerned not to disturb. A project whose
    // every source kind has a rule should see nothing about coverage gaps.
    let p = Project::new("clean");
    std::fs::remove_file(p.dir.join("src/theme.scss")).ok();
    std::fs::remove_file(p.dir.join("src/mixins.scss")).ok();
    std::fs::remove_file(p.dir.join("src/reset.css")).ok();
    std::fs::remove_file(p.dir.join("src/Button.tsx")).ok();
    let rule = p.dir.join("catch.yml").to_string_lossy().to_string();
    let payload = json(&p.census("s", &rule));
    assert_eq!(payload["sites_found"], 1);
    for key in [
        "WARNING",
        "WARNING_uncovered_extensions",
        "WARNING_unparseable",
        "WARNING_unparseable_extensions",
        "WARNING_unknown_rule_languages",
    ] {
        assert!(payload.get(key).is_none(), "{key} appeared on a project with no gaps");
    }
}

#[test]
fn the_menu_and_the_extension_map_agree_in_both_directions() {
    // Forwards: nothing is offered that has no extensions, so no rule can load
    // and then be reported as absent from the map.
    for name in rules::SUPPORTED_LANGUAGES {
        assert!(
            languages::extensions_for(name).is_some(),
            "{name} is offered and has no extensions"
        );
    }
    // Backwards: everything in the map is either offered or declared
    // unparseable. A third state would be a language the tool counts as source
    // and says nothing about.
    for (name, _) in languages::LANGUAGE_EXTENSIONS {
        assert!(
            rules::SUPPORTED_LANGUAGES.contains(name) || languages::UNPARSEABLE_LANGUAGES.contains(name),
            "{name} is in the extension map but is neither offered nor declared unparseable"
        );
    }
}
