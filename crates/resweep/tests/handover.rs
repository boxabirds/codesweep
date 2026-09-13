//! A live sweep handed from the replaced tool to the port, and the state
//! machine asserted at both ends.
//!
//! Real storage throughout. A mocked one could not answer the only question
//! this file exists for: whether a session's work survives the change.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use resweep::ledger;

/// The last commit at which the replaced tool existed, and the path it lived
/// at then, which is the retired name: the rename came after this commit.
const REPLACED_TOOL_COMMIT: &str = "80187f6";
const REPLACED_TOOL_PATH: &str = "bin/codesweep";
/// It predates the rename, so it reads its own override variable and writes
/// under its own temporary namespace.
const RETIRED_OVERRIDE: &str = "CODESWEEP_SESSION_ID";
const RETIRED_NAMESPACE: &str = "codesweep";

/// Where a given session's ledger lives, built from the session name rather
/// than read from the environment.
///
/// `ledger::ledger_path` reads the session out of the process environment,
/// which is correct for a command that runs once but wrong for tests: cases in
/// one binary share that environment and run in parallel, so one case setting
/// the variable changes the path another case is about to compute.
fn ledger_for(session: &str, root: &Path) -> PathBuf {
    session_dir_for(session).join(format!("{}.db", ledger::repo_slug(root)))
}

fn session_dir_for(session: &str) -> PathBuf {
    std::env::temp_dir().join(ledger::TEMP_NAMESPACE).join(session)
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn port() -> PathBuf {
    repo().join("target/release/resweep")
}

fn replaced_tool() -> Option<PathBuf> {
    if Command::new("python3").arg("--version").output().is_err() {
        return None;
    }
    if Command::new("ast-grep").arg("--version").output().is_err() {
        return None;
    }
    let path = std::env::temp_dir().join(format!("resweep-replaced-{}", std::process::id()));
    if path.exists() {
        return Some(path);
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(repo())
        .arg("show")
        .arg(format!("{REPLACED_TOOL_COMMIT}:{REPLACED_TOOL_PATH}"))
        .output()
        .ok()?;
    if !out.status.success() || out.stdout.is_empty() {
        return None;
    }
    // Renamed into place rather than written in place: parallel cases must see
    // the file whole or not at all.
    let staged = path.with_extension(format!("{:?}", std::thread::current().id()));
    std::fs::write(&staged, &out.stdout).ok()?;
    std::fs::rename(&staged, &path).ok()?;
    Some(path)
}

struct Sweep {
    dir: PathBuf,
    session: String,
}

impl Sweep {
    fn new(label: &str) -> Self {
        let session = format!("handover-{label}-{}", std::process::id());
        let dir = std::env::temp_dir().join(format!("resweep-{session}"));
        std::fs::create_dir_all(dir.join("src")).expect("a fixture directory");
        std::fs::write(
            dir.join("catch.yml"),
            "id: ts-catch\nlanguage: typescript\nrule:\n  kind: catch_clause\n",
        )
        .expect("a rule");
        Self { dir, session }
    }

    fn write(&self, name: &str, body: &str) {
        std::fs::write(self.dir.join("src").join(name), body).expect("a source file");
    }

    fn by_port(&self, args: &[&str]) -> Output {
        Command::new(port())
            .args(args)
            .arg("--root")
            .arg(&self.dir)
            .env(ledger::SESSION_ENV_OVERRIDE, &self.session)
            .output()
            .expect("the port runs")
    }

    fn by_replaced(&self, tool: &Path, args: &[&str]) -> Output {
        Command::new("python3")
            .arg(tool)
            .args(args)
            .arg("--root")
            .arg(&self.dir)
            .env(RETIRED_OVERRIDE, &self.session)
            .output()
            .expect("the replaced tool runs")
    }

    /// The two keep their ledgers under different names, because the namespace
    /// carries the tool's name and the name changed at the rename. Carrying
    /// the file across makes the comparison about the ledger rather than about
    /// where each tool keeps one.
    fn carry(&self, to_port: bool) {
        let theirs = std::env::temp_dir()
            .join(RETIRED_NAMESPACE)
            .join(&self.session)
            .join(format!("{}.db", ledger::repo_slug(&self.dir)));
        let ours = ledger_for(&self.session, &self.dir);
        let (from, to) = if to_port { (&theirs, &ours) } else { (&ours, &theirs) };
        std::fs::create_dir_all(to.parent().expect("a session directory")).expect("a directory");
        std::fs::copy(from, to).unwrap_or_else(|e| panic!("carrying {from:?} to {to:?}: {e}"));
    }
}

impl Drop for Sweep {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
        for namespace in [ledger::TEMP_NAMESPACE, RETIRED_NAMESPACE] {
            std::fs::remove_dir_all(std::env::temp_dir().join(namespace).join(&self.session)).ok();
        }
    }
}

fn json(out: &Output) -> serde_json::Value {
    let body = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(&body).unwrap_or_else(|e| {
        panic!("not JSON: {e}\nstdout: {body}\nstderr: {}", String::from_utf8_lossy(&out.stderr))
    })
}

#[test]
fn a_sweep_started_by_the_replaced_tool_is_finished_by_the_port() {
    let Some(tool) = replaced_tool() else {
        eprintln!("SKIP: the replaced tool needs an interpreter and a matching engine, and one is absent");
        return;
    };
    let s = Sweep::new("finish");
    for name in ["a.ts", "b.ts", "c.ts", "d.ts"] {
        s.write(name, "export function f() { try { x(); } catch (e) { return []; } }\n");
    }
    let rule = s.dir.join("catch.yml").to_string_lossy().to_string();

    // Half a sweep, done by the tool that is going away.
    let census = s.by_replaced(&tool, &[
        "census", "s", "--rule", &rule, "--scope", "src", "--question", "swallowed?",
    ]);
    assert!(census.status.success(), "{}", String::from_utf8_lossy(&census.stderr));
    assert_eq!(json(&census)["sites_found"], 4);

    let batch = json(&s.by_replaced(&tool, &["next", "s", "--limit", "2"]));
    let judged: Vec<String> = batch["sites"]
        .as_array()
        .expect("a batch")
        .iter()
        .map(|x| x["site_id"].as_str().expect("a site id").to_string())
        .collect();
    assert_eq!(judged.len(), 2);
    for id in &judged {
        let out = s.by_replaced(&tool, &[
            "verdict", "s", "--site", id, "--verdict", "violation",
            "--note", "returns an empty array", "--method", "read the source",
        ]);
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }
    let midway = json(&s.by_replaced(&tool, &["status", "s"]));
    assert_eq!(midway["coverage"]["judged"], 2);
    assert_eq!(midway["coverage"]["unjudged"], 2);

    // The other half, done by the port.
    s.carry(true);
    let taken_up = json(&s.by_port(&["status", "s"]));
    assert_eq!(taken_up["coverage"], midway["coverage"], "counts changed hands");
    assert_eq!(taken_up["question"], midway["question"]);
    assert_eq!(taken_up["scope"], midway["scope"]);
    assert_eq!(taken_up["rules"], midway["rules"], "the recorded rules changed hands");

    let rest = json(&s.by_port(&["next", "s", "--limit", "10"]));
    let remaining: Vec<String> = rest["sites"]
        .as_array()
        .expect("a batch")
        .iter()
        .map(|x| x["site_id"].as_str().expect("a site id").to_string())
        .collect();
    assert_eq!(remaining.len(), 2, "the port offered a different number of sites");
    for id in &remaining {
        assert!(!judged.contains(id), "a site judged earlier came back as unjudged");
        let out = s.by_port(&[
            "verdict", "s", "--site", id, "--verdict", "pass",
            "--note", "rethrows", "--method", "read the source",
        ]);
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }

    let done = json(&s.by_port(&["status", "s"]));
    assert_eq!(done["coverage"]["judged"], 4);
    assert_eq!(done["coverage"]["unjudged"], 0);
    assert_eq!(done["complete"], true);
    assert_eq!(done["coverage"]["by_verdict"]["violation"], 2, "an earlier verdict was lost");
    assert_eq!(done["coverage"]["by_verdict"]["pass"], 2);

    // And back again, because someone may not upgrade in one step.
    s.carry(false);
    let seen_again = json(&s.by_replaced(&tool, &["status", "s"]));
    assert_eq!(seen_again["coverage"]["judged"], 4);
    assert_eq!(seen_again["complete"], true);
}

#[test]
fn no_ledger_becomes_one_sweep_with_its_rules_recorded() {
    let s = Sweep::new("open");
    s.write("a.ts", "export function a() { try { x(); } catch (e) { return []; } }\n");
    let rule = s.dir.join("catch.yml").to_string_lossy().to_string();
    assert!(!ledger_for(&s.session, &s.dir).exists());

    s.by_port(&["census", "s", "--rule", &rule, "--scope", "src", "--question", "swallowed?"]);
    assert!(ledger_for(&s.session, &s.dir).exists());

    let status = json(&s.by_port(&["status", "s"]));
    let rules = status["rules"].as_array().expect("recorded rules");
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0]["rule_id"], "ts-catch");
    assert_eq!(rules[0]["language"], "typescript");
    // The source hash is recorded so the claim stays checkable after the rule
    // file has moved or gone.
    assert!(rules[0]["source_hash"].as_str().expect("a hash").len() > 0);
}

#[test]
fn a_rerun_with_a_different_question_is_refused_and_quotes_the_recorded_one() {
    // Changing the question changes what every recorded verdict means.
    let s = Sweep::new("question");
    s.write("a.ts", "export function a() { try { x(); } catch (e) { return []; } }\n");
    let rule = s.dir.join("catch.yml").to_string_lossy().to_string();
    s.by_port(&["census", "s", "--rule", &rule, "--scope", "src", "--question", "swallowed?"]);

    let out = s.by_port(&[
        "census", "s", "--rule", &rule, "--scope", "src", "--question", "something else",
    ]);
    assert!(!out.status.success());
    let message = String::from_utf8_lossy(&out.stderr);
    assert!(message.contains("swallowed?"), "the recorded question was not quoted: {message}");
    assert!(message.contains("Start a new sweep"), "{message}");
    assert!(String::from_utf8_lossy(&out.stdout).trim().is_empty());
}

#[test]
fn an_unwritable_session_directory_reports_the_path_and_writes_nothing() {
    let s = Sweep::new("unwritable");
    s.write("a.ts", "export function a() { try { x(); } catch (e) { return []; } }\n");
    let rule = s.dir.join("catch.yml").to_string_lossy().to_string();

    let session_dir = session_dir_for(&s.session);
    std::fs::create_dir_all(&session_dir).expect("a directory");
    let mut perms = std::fs::metadata(&session_dir).expect("readable").permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o500);
    }
    std::fs::set_permissions(&session_dir, perms).expect("permissions set");

    let out = s.by_port(&["census", "s", "--rule", &rule, "--scope", "src", "--question", "q"]);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut back = std::fs::metadata(&session_dir).expect("readable").permissions();
        back.set_mode(0o700);
        std::fs::set_permissions(&session_dir, back).ok();
    }

    assert!(!out.status.success(), "an unwritable ledger location was not reported");
    let message = String::from_utf8_lossy(&out.stderr);
    assert!(!message.trim().is_empty(), "it failed without saying why");
    assert!(
        !ledger_for(&s.session, &s.dir).exists(),
        "a ledger was written where it could not be"
    );
}

#[test]
fn the_repository_being_swept_is_never_written_to() {
    let s = Sweep::new("readonly");
    s.write("a.ts", "export function a() { try { x(); } catch (e) { return []; } }\n");
    let rule = s.dir.join("catch.yml").to_string_lossy().to_string();

    let listing = |dir: &Path| -> Vec<String> {
        let mut names: Vec<String> = ignore::WalkBuilder::new(dir)
            .hidden(false)
            .standard_filters(false)
            .build()
            .filter_map(Result::ok)
            .filter_map(|e| e.path().strip_prefix(dir).ok().map(|p| p.display().to_string()))
            .collect();
        names.sort();
        names
    };
    let before = listing(&s.dir);

    s.by_port(&["census", "s", "--rule", &rule, "--scope", "src", "--question", "swallowed?"]);
    let site = json(&s.by_port(&["next", "s"]))["sites"][0]["site_id"]
        .as_str()
        .expect("a site id")
        .to_string();
    s.by_port(&["verdict", "s", "--site", &site, "--verdict", "pass", "--note", "n", "--method", "m"]);
    for args in [
        vec!["status", "s"], vec!["manifest", "s"], vec!["list"],
        vec!["surfaces", "s"], vec!["recheck", "s"], vec!["report", "s"],
        vec!["show", "s", "--site", &site],
    ] {
        s.by_port(&args);
    }

    assert_eq!(before, listing(&s.dir), "the tool left something in the repository");
}
