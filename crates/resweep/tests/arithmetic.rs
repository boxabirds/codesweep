//! Site identity under change, and the subtraction that turns counts into a
//! coverage claim.
//!
//! Driven through the built binary rather than the library, because the claim
//! that matters is the one an operator reads, and the payload is where the
//! arithmetic and the completeness verdict meet.

use std::path::{Path, PathBuf};
use std::process::Command;

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
        let session = format!("arith-{label}-{}", std::process::id());
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

    fn run(&self, args: &[&str]) -> serde_json::Value {
        let out = Command::new(binary())
            .args(args)
            .arg("--root")
            .arg(&self.dir)
            .env(ledger::SESSION_ENV_OVERRIDE, &self.session)
            .output()
            .expect("the tool runs");
        let body = String::from_utf8_lossy(&out.stdout);
        serde_json::from_str(&body).unwrap_or_else(|e| {
            panic!(
                "{args:?} did not produce JSON: {e}\nstdout: {body}\nstderr: {}",
                String::from_utf8_lossy(&out.stderr)
            )
        })
    }

    fn census(&self) -> serde_json::Value {
        let rule = self.dir.join("catch.yml");
        self.run(&[
            "census",
            "s",
            "--rule",
            rule.to_str().unwrap(),
            "--scope",
            "src",
            "--question",
            "swallowed?",
        ])
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
        std::fs::remove_dir_all(std::env::temp_dir().join(ledger::TEMP_NAMESPACE).join(&self.session)).ok();
    }
}

fn site_ids(payload: &serde_json::Value) -> Vec<String> {
    payload["sites"]
        .as_array()
        .expect("a batch of sites")
        .iter()
        .map(|s| s["site_id"].as_str().expect("a site id").to_string())
        .collect()
}

#[test]
fn reformatting_a_site_keeps_its_verdict() {
    // The reason the text is collapsed before hashing. A formatter running
    // between two censuses is not a change of site, and a verdict recorded
    // before it still applies.
    let f = Fixture::new("reformat");
    f.write("a.ts", "export function a() { try { x(); } catch (e) { return []; } }\n");
    f.census();
    let before = site_ids(&f.run(&["next", "s"]));
    assert_eq!(before.len(), 1);
    f.run(&[
        "verdict", "s", "--site", &before[0], "--verdict", "violation",
        "--note", "returns an empty array", "--method", "read the source",
    ]);

    // Same clause, laid out differently.
    f.write(
        "a.ts",
        "export function a() {\n  try {\n    x();\n  } catch (e) {\n    return [];\n  }\n}\n",
    );
    let again = f.census();
    assert_eq!(again["sites_new"], 0, "a reformat created a new site");
    let status = f.run(&["status", "s"]);
    assert_eq!(status["coverage"]["unjudged"], 0, "the verdict was lost to a reformat");
    assert_eq!(status["coverage"]["judged"], 1);
}

#[test]
fn a_materially_changed_site_returns_as_unjudged() {
    // The other half, and the one that protects the claim. A clause that now
    // does something different has not been judged, whatever was decided about
    // what used to be there.
    let f = Fixture::new("changed");
    f.write("a.ts", "export function a() { try { x(); } catch (e) { return []; } }\n");
    f.census();
    let before = site_ids(&f.run(&["next", "s"]));
    f.run(&[
        "verdict", "s", "--site", &before[0], "--verdict", "violation",
        "--note", "returns an empty array", "--method", "read the source",
    ]);
    assert_eq!(f.run(&["status", "s"])["coverage"]["unjudged"], 0);

    f.write("a.ts", "export function a() { try { x(); } catch (e) { throw e; } }\n");
    let again = f.census();
    assert_eq!(again["sites_new"], 1, "a changed clause is a new site");
    assert_eq!(again["sites_departed"], 1, "the old one departed");
    let status = f.run(&["status", "s"]);
    assert_eq!(status["coverage"]["unjudged"], 1, "the changed site needs judging again");
    let after = site_ids(&f.run(&["next", "s"]));
    assert_ne!(after[0], before[0], "the identity did not change with the text");
}

#[test]
fn a_departed_site_is_marked_rather_than_deleted() {
    // Deleting it would lose the verdict and the fact that the site once
    // existed, which is what a later reader needs to tell a fixed violation
    // from one nobody ever looked at.
    let f = Fixture::new("departed");
    f.write("a.ts", "export function a() { try { x(); } catch (e) { return []; } }\n");
    f.census();
    let before = site_ids(&f.run(&["next", "s"]));
    f.run(&[
        "verdict", "s", "--site", &before[0], "--verdict", "violation",
        "--note", "returns an empty array", "--method", "read the source",
    ]);
    f.write("a.ts", "export function a() { return x(); }\n");
    let again = f.census();
    assert_eq!(again["sites_departed"], 1);
    assert_eq!(f.run(&["status", "s"])["coverage"]["live_sites"], 0);
    // Still there, still judged, just no longer live.
    let shown = f.run(&["show", "s", "--site", &before[0]]);
    assert_eq!(shown["state"], "gone");
    assert_eq!(shown["verdict"]["verdict"], "violation");
}

#[test]
fn a_batch_limit_of_one_hands_out_one() {
    let f = Fixture::new("limit-one");
    for (name, _) in [("a.ts", 0), ("b.ts", 0), ("c.ts", 0)] {
        f.write(name, "export function f() { try { x(); } catch (e) { return []; } }\n");
    }
    f.census();
    assert_eq!(site_ids(&f.run(&["next", "s", "--limit", "1"])).len(), 1);
}

#[test]
fn a_batch_limit_larger_than_the_queue_pads_nothing() {
    let f = Fixture::new("limit-big");
    for name in ["a.ts", "b.ts"] {
        f.write(name, "export function f() { try { x(); } catch (e) { return []; } }\n");
    }
    f.census();
    let batch = site_ids(&f.run(&["next", "s", "--limit", "500"]));
    assert_eq!(batch.len(), 2, "the batch was padded or truncated");
    // And the tool says how many are left, rather than leaving it to be
    // inferred from the size of what came back.
    assert_eq!(f.run(&["next", "s", "--limit", "500"])["remaining_after_this_batch"], 0);
}

#[test]
fn coverage_is_live_minus_judged_at_every_step() {
    let f = Fixture::new("subtraction");
    for name in ["a.ts", "b.ts", "c.ts"] {
        f.write(name, "export function f() { try { x(); } catch (e) { return []; } }\n");
    }
    f.census();
    let ids = site_ids(&f.run(&["next", "s", "--limit", "10"]));
    assert_eq!(ids.len(), 3);

    for (judged, id) in ids.iter().enumerate() {
        let status = f.run(&["status", "s"]);
        assert_eq!(status["coverage"]["live_sites"], 3);
        assert_eq!(status["coverage"]["judged"], judged);
        assert_eq!(status["coverage"]["unjudged"], 3 - judged);
        // Not complete while anything is outstanding, and the reason says how
        // many rather than only that something is missing.
        assert_eq!(status["complete"], false);
        let reasons = status["incomplete_because"].as_array().expect("reasons");
        assert!(
            reasons.iter().any(|r| r.as_str().unwrap_or_default().contains(&format!("{} enumerated sites have no verdict", 3 - judged))),
            "reasons did not say how many were outstanding: {reasons:?}"
        );
        f.run(&[
            "verdict", "s", "--site", id, "--verdict", "pass",
            "--note", "rethrows", "--method", "read the source",
        ]);
    }

    let done = f.run(&["status", "s"]);
    assert_eq!(done["coverage"]["unjudged"], 0);
    assert_eq!(done["complete"], true);
    assert_eq!(done["incomplete_because"].as_array().expect("reasons").len(), 0);
}

#[test]
fn an_empty_sweep_is_not_complete() {
    // Zero of zero judged is arithmetically complete and means nothing. A
    // sweep that enumerated nothing has nothing to be complete about, and
    // reporting it as complete is the most confident way to be wrong.
    let f = Fixture::new("empty");
    f.write("a.ts", "export function a() { return 1; }\n");
    f.census();
    let status = f.run(&["status", "s"]);
    assert_eq!(status["coverage"]["live_sites"], 0);
    assert_eq!(status["complete"], false);
    let reasons = status["incomplete_because"].as_array().expect("reasons");
    assert!(
        reasons.iter().any(|r| r.as_str().unwrap_or_default().contains("nothing to be complete about")),
        "{reasons:?}"
    );
}

#[test]
fn every_reason_is_reported_at_once_and_not_one_at_a_time() {
    // An implementation that stops at the first failure hands the operator one
    // reason, they fix it, and they walk into the next one.
    let f = Fixture::new("reasons");
    f.write("a.ts", "export function a() { try { x(); } catch (e) { return []; } }\n");
    // A file no rule can reach, so the uncovered-extension reason applies too.
    std::fs::write(f.dir.join("src/styles.css"), ".a { color: #fff; }\n").expect("a css file");
    f.census();
    let status = f.run(&["status", "s"]);
    let reasons: Vec<String> = status["incomplete_because"]
        .as_array()
        .expect("reasons")
        .iter()
        .map(|r| r.as_str().unwrap_or_default().to_string())
        .collect();
    assert!(reasons.len() >= 2, "only one reason was given: {reasons:?}");
    assert!(reasons.iter().any(|r| r.contains("no verdict")), "{reasons:?}");
    assert!(reasons.iter().any(|r| r.contains("no rule can reach")), "{reasons:?}");
}
