//! The argument surface, and every refusal it owes.
//!
//! The refusals are compared byte for byte against the recording by
//! tests/capture-reference.sh. What is here is what a recording cannot show:
//! that the root option means the same thing in all three positions it can
//! occupy, and that a refusal refuses rather than merely printing something.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use resweep::ledger;

fn binary() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/release/resweep")
}

struct Fixture {
    dir: PathBuf,
    session: String,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let session = format!("surface-{label}-{}", std::process::id());
        let dir = std::env::temp_dir().join(format!("resweep-{session}"));
        std::fs::create_dir_all(dir.join("src")).expect("a fixture directory");
        std::fs::write(
            dir.join("src/a.ts"),
            "export function a() { try { x(); } catch (e) { return []; } }\n",
        )
        .expect("a source file");
        std::fs::write(
            dir.join("catch.yml"),
            "id: ts-catch\nlanguage: typescript\nrule:\n  kind: catch_clause\n",
        )
        .expect("a rule");
        Self { dir, session }
    }

    /// Run with the arguments exactly as given, from a working directory that
    /// is deliberately not the fixture, so a command that silently uses the
    /// working directory is caught rather than accidentally right.
    fn run_from(&self, cwd: &Path, args: &[&str]) -> Output {
        Command::new(binary())
            .args(args)
            .current_dir(cwd)
            .env(ledger::SESSION_ENV_OVERRIDE, &self.session)
            .output()
            .expect("the tool runs")
    }

    fn run(&self, args: &[&str]) -> Output {
        self.run_from(Path::new("/"), args)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
        std::fs::remove_dir_all(std::env::temp_dir().join(ledger::TEMP_NAMESPACE).join(&self.session)).ok();
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

#[test]
fn the_root_option_means_the_same_thing_before_and_after_the_subcommand() {
    let f = Fixture::new("root-position");
    let root = f.dir.to_str().unwrap().to_string();
    let rule = f.dir.join("catch.yml").to_string_lossy().to_string();

    let before = f.run(&[
        "--root", &root, "census", "s", "--rule", &rule, "--scope", "src",
        "--question", "swallowed?",
    ]);
    assert!(before.status.success(), "{}", stderr(&before));

    // The same census again, with the option on the other side. A rerun over
    // an unchanged tree finds the same sites and adds none, which only holds
    // if both invocations reached the same repository and the same ledger.
    let after = f.run(&[
        "census", "s", "--rule", &rule, "--scope", "src", "--root", &root,
    ]);
    assert!(after.status.success(), "{}", stderr(&after));

    let first: serde_json::Value = serde_json::from_str(&stdout(&before)).expect("JSON");
    let second: serde_json::Value = serde_json::from_str(&stdout(&after)).expect("JSON");
    assert_eq!(first["sites_found"], second["sites_found"]);
    assert_eq!(second["sites_new"], 0, "the second invocation swept a different tree");
    assert_eq!(first["scope"], second["scope"]);
}

#[test]
fn the_root_option_absent_means_the_working_directory() {
    let f = Fixture::new("root-absent");
    let rule = f.dir.join("catch.yml").to_string_lossy().to_string();
    // Run from inside the fixture with no root at all.
    let out = f.run_from(&f.dir, &[
        "census", "s", "--rule", &rule, "--scope", "src", "--question", "swallowed?",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    let payload: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("JSON");
    assert_eq!(payload["sites_found"], 1);
    assert_eq!(payload["scope"], "src");
}

#[test]
fn a_relative_scope_is_read_against_the_root_and_not_the_working_directory() {
    // The failure this prevents is the quiet one: the same command sweeping a
    // different tree depending on where it was run, and printing its coverage
    // arithmetic with full confidence either way.
    let f = Fixture::new("relative-scope");
    let root = f.dir.to_str().unwrap().to_string();
    let rule = f.dir.join("catch.yml").to_string_lossy().to_string();

    // A decoy: a directory called src in the working directory, holding twice
    // as many sites. If the scope resolved against the working directory the
    // count would come from here.
    let elsewhere = std::env::temp_dir().join(format!("resweep-decoy-{}", std::process::id()));
    std::fs::create_dir_all(elsewhere.join("src")).expect("a decoy directory");
    std::fs::write(
        elsewhere.join("src/decoy.ts"),
        "function a(){try{x();}catch(e){return [];}}\nfunction b(){try{x();}catch(e){return [];}}\n",
    )
    .expect("a decoy file");

    let out = f.run_from(&elsewhere, &[
        "census", "s", "--rule", &rule, "--scope", "src", "--question", "swallowed?",
        "--root", &root,
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    let payload: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("JSON");
    assert_eq!(payload["sites_found"], 1, "the scope resolved against the working directory");
    std::fs::remove_dir_all(&elsewhere).ok();
}

#[test]
fn every_refusal_refuses() {
    // The wording is compared against the recording elsewhere. What matters
    // here is that each of these leaves a non-zero status and writes nothing
    // to standard output, so a caller reading the payload cannot mistake a
    // refusal for an empty result.
    let f = Fixture::new("refusals");
    let root = f.dir.to_str().unwrap().to_string();
    let rule = f.dir.join("catch.yml").to_string_lossy().to_string();
    f.run(&["--root", &root, "census", "s", "--rule", &rule, "--scope", "src", "--question", "q"]);

    let site = {
        let out = f.run(&["--root", &root, "next", "s"]);
        let payload: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("JSON");
        payload["sites"][0]["site_id"].as_str().expect("a site id").to_string()
    };

    let cases: Vec<(&str, Vec<String>)> = vec![
        ("an unknown subcommand", vec!["nosuchcommand".into()]),
        ("a missing required argument", vec![
            "--root".into(), root.clone(), "verdict".into(), "s".into(),
            "--verdict".into(), "pass".into(), "--note".into(), "n".into(),
        ]),
        ("a verdict outside the permitted set", vec![
            "--root".into(), root.clone(), "verdict".into(), "s".into(),
            "--site".into(), site.clone(), "--verdict".into(), "maybe".into(),
            "--note".into(), "n".into(),
        ]),
        ("a verdict with an empty note", vec![
            "--root".into(), root.clone(), "verdict".into(), "s".into(),
            "--site".into(), site.clone(), "--verdict".into(), "pass".into(),
            "--note".into(), "".into(),
        ]),
        ("a verdict with a note of only spaces", vec![
            "--root".into(), root.clone(), "verdict".into(), "s".into(),
            "--site".into(), site.clone(), "--verdict".into(), "pass".into(),
            "--note".into(), "   ".into(),
        ]),
        ("an unknown sweep", vec!["--root".into(), root.clone(), "status".into(), "nope".into()]),
        ("a scope that does not exist", vec![
            "--root".into(), root.clone(), "census".into(), "other".into(),
            "--rule".into(), rule.clone(), "--scope".into(), "nowhere".into(),
            "--question".into(), "q".into(),
        ]),
        ("an unknown site", vec![
            "--root".into(), root.clone(), "show".into(), "s".into(),
            "--site".into(), "0000000000000000".into(),
        ]),
        ("a rule file that is not there", vec![
            "--root".into(), root.clone(), "census".into(), "other".into(),
            "--rule".into(), "/no/such/rule.yml".into(), "--scope".into(), "src".into(),
            "--question".into(), "q".into(),
        ]),
    ];

    for (label, args) in cases {
        let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = f.run(&borrowed);
        assert!(!out.status.success(), "{label} did not refuse");
        assert!(
            stdout(&out).trim().is_empty(),
            "{label} wrote to standard output: {}",
            stdout(&out)
        );
        assert!(!stderr(&out).trim().is_empty(), "{label} refused without saying why");
    }

    // And the one that must not be lost: after all that, nothing was recorded.
    let status = f.run(&["--root", &root, "status", "s"]);
    let payload: serde_json::Value = serde_json::from_str(&stdout(&status)).expect("JSON");
    assert_eq!(payload["coverage"]["judged"], 0, "a refused verdict was written anyway");
}

#[test]
fn the_subcommand_list_is_the_one_the_replaced_tool_offered() {
    // No command was added. A rewrite that also adds one cannot be verified
    // against the thing it replaces.
    let out = Command::new(binary()).arg("nosuchcommand").output().expect("the tool runs");
    let message = stderr(&out);
    for name in [
        "census", "next", "verdict", "surfaces", "recheck", "status", "report",
        "manifest", "show", "list",
    ] {
        assert!(message.contains(name), "the refusal did not offer {name}: {message}");
    }
    // Ten and no more. Counted from the braced list the refusal prints.
    let listed = message
        .split('{')
        .nth(1)
        .and_then(|rest| rest.split('}').next())
        .expect("the refusal lists the commands");
    assert_eq!(listed.split(',').count(), 10, "the surface grew or shrank: {listed}");
}
