//! The ledger and the site identity, against the tool being replaced.
//!
//! A test that only showed identities were stable would pass just as happily
//! against a drifted construction. Every comparison here is against a value
//! the Python tool produces on the spot, so a drift fails rather than being
//! re-recorded as the new truth.

use std::path::{Path, PathBuf};
use std::process::Command;

use resweep::ledger;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Run a snippet against the Python tool's own definitions, so the expected
/// value comes from the implementation rather than from a description of it.
fn python(snippet: &str) -> String {
    let tool = repo().join("bin/resweep");
    let program = format!(
        "import importlib.util, sys\n\
         spec = importlib.util.spec_from_loader('tool', None)\n\
         mod = importlib.util.module_from_spec(spec)\n\
         mod.__dict__['__name__'] = 'tool'\n\
         src = open({tool:?}).read()\n\
         # The tool runs its command line at import, so stop before that.\n\
         src = src.split('def main(')[0]\n\
         exec(compile(src, {tool:?}, 'exec'), mod.__dict__)\n\
         {snippet}\n"
    );
    let out = Command::new("python3")
        .arg("-c")
        .arg(&program)
        .output()
        .expect("python3 runs");
    assert!(
        out.status.success(),
        "the tool would not load: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn the_schema_version_is_the_same_number() {
    assert_eq!(
        python("print(mod.SCHEMA_VERSION)"),
        ledger::SCHEMA_VERSION.to_string()
    );
}

#[test]
fn the_constants_that_reach_disk_are_the_same() {
    assert_eq!(python("print(mod.SITE_ID_HEX_CHARS)"), ledger::SITE_ID_HEX_CHARS.to_string());
    assert_eq!(python("print(mod.TEMP_NAMESPACE)"), ledger::TEMP_NAMESPACE);
    assert_eq!(python("print(mod.SESSION_ENV)"), ledger::SESSION_ENV);
    assert_eq!(python("print(mod.SESSION_ENV_OVERRIDE)"), ledger::SESSION_ENV_OVERRIDE);
    assert_eq!(python("print(mod.STATE_LIVE)"), ledger::STATE_LIVE);
    assert_eq!(python("print(mod.STATE_GONE)"), ledger::STATE_GONE);
    assert_eq!(
        python("print(','.join(mod.VERDICTS))"),
        ledger::VERDICTS.join(",")
    );
    assert_eq!(
        python("print(mod.SESSION_RETENTION_SECONDS)"),
        ledger::SESSION_RETENTION.as_secs().to_string()
    );
}

#[test]
fn whitespace_is_collapsed_the_same_way() {
    // Every shape a match can arrive in: a run of spaces, a newline and
    // indentation, a tab, leading and trailing space, and a lone word.
    let cases = [
        "catch (e) { return []; }",
        "catch (e) {\n    throw e;\n  }",
        "catch\t(e)\t{}",
        "   padded   ",
        "single",
        "a\n\n\nb",
        "",
    ];
    for case in cases {
        // Delimited rather than quoted. Python's repr uses single quotes and
        // Rust's uses double, so comparing the two reprs compares the quoting
        // as well as the value. The markers make leading and trailing space
        // visible in a failure without introducing that difference.
        let expected = python(&format!("print('[' + mod.normalise({case:?}) + ']')"));
        let mine = format!("[{}]", ledger::normalise(case));
        assert_eq!(expected, mine, "normalising {case:?}");
    }
}

#[test]
fn site_identities_are_the_same_strings() {
    // Not merely stable: the same. A stable but different construction orphans
    // every verdict from a prior session while the arithmetic still adds up.
    let cases: &[(&str, &str, usize, &str)] = &[
        ("ts-catch-clause", "src/a.ts", 0, "catch (e) { return []; }"),
        ("ts-catch-clause", "src/a.ts", 1, "catch (e) { return []; }"),
        ("ts-catch-clause", "src/b.ts", 0, "catch (e) {\n    throw e;\n  }"),
        ("tsx-catch-clause", "src/App.tsx", 0, "catch { return null; }"),
        ("ts-logical-default", "lib/deep/nested/path.ts", 12, "a || b"),
        ("rule-with-unicode", "src/résumé.ts", 0, "catch (é) { }"),
    ];
    for (rule, file, ordinal, text) in cases {
        let expected = python(&format!(
            "print(mod.site_identity({rule:?}, {file:?}, {ordinal}, {text:?}))"
        ));
        assert_eq!(
            expected,
            ledger::site_identity(rule, file, *ordinal, text),
            "identity of {rule} {file} #{ordinal}"
        );
    }
}

#[test]
fn a_reformatted_site_keeps_its_identity() {
    // The reason the text is collapsed at all. Reformatting is not a change of
    // site, and a verdict recorded before a formatter ran still applies.
    let before = ledger::site_identity("r", "a.ts", 0, "catch (e) { return []; }");
    let after = ledger::site_identity("r", "a.ts", 0, "catch (e) {\n  return [];\n}");
    assert_eq!(before, after);
}

#[test]
fn the_repository_slug_is_the_same_filename() {
    let cases = ["/tmp/fixture", "/tmp/some.dir/with spaces", "/"];
    for case in cases {
        let expected = python(&format!("print(mod.repo_slug({case:?}))"));
        assert_eq!(expected, ledger::repo_slug(Path::new(case)), "slug of {case}");
    }
}

#[test]
fn a_ledger_written_by_python_is_read_by_rust_and_back_again() {
    let session = format!("ledgerport-{}", std::process::id());
    let work = std::env::temp_dir().join(format!("resweep-ledger-{}", std::process::id()));
    let src = work.join("src");
    std::fs::create_dir_all(&src).expect("a fixture directory");
    std::fs::write(
        src.join("a.ts"),
        "export function a() { try { x(); } catch (e) { return []; } }\n",
    )
    .expect("a fixture file");
    std::fs::write(
        work.join("catch.yml"),
        "id: ts-catch\nlanguage: typescript\nrule:\n  kind: catch_clause\n",
    )
    .expect("a rule");
    for args in [
        vec!["init", "-q", "."],
        vec!["-c", "user.email=t@t", "-c", "user.name=t", "add", "-A"],
        vec!["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qm", "fixture"],
    ] {
        Command::new("git").args(&args).current_dir(&work).output().ok();
    }

    let tool = repo().join("bin/resweep");
    let ran = Command::new(&tool)
        .args([
            "census",
            "s",
            "--rule",
            work.join("catch.yml").to_str().unwrap(),
            "--scope",
            "src",
            "--question",
            "swallowed?",
            "--root",
            work.to_str().unwrap(),
        ])
        .env(ledger::SESSION_ENV_OVERRIDE, &session)
        .output()
        .expect("the tool runs");
    assert!(ran.status.success(), "{}", String::from_utf8_lossy(&ran.stderr));

    // The same environment, so the Rust side computes the same path rather
    // than being told where to look.
    std::env::set_var(ledger::SESSION_ENV_OVERRIDE, &session);
    let path = ledger::ledger_path(&work).expect("a ledger path");
    assert!(path.exists(), "the Rust side computed {path:?}, which is not there");

    let conn = ledger::connect(&work, false).expect("the ledger opens");
    let version: String = conn
        .query_row("SELECT value FROM meta WHERE key = 'schema_version'", [], |r| r.get(0))
        .expect("the version is recorded");
    assert_eq!(version, ledger::SCHEMA_VERSION.to_string());
    let sites: i64 = conn
        .query_row("SELECT count(*) FROM site WHERE sweep = 's'", [], |r| r.get(0))
        .expect("sites are counted");
    assert_eq!(sites, 1);

    // Rust writes, Python reads. A one-way check would miss a column the port
    // added or a value it stored in a shape the other cannot parse.
    let site_id: String = conn
        .query_row("SELECT site_id FROM site WHERE sweep = 's'", [], |r| r.get(0))
        .expect("the site has an id");
    conn.execute(
        "INSERT INTO verdict (sweep, site_id, verdict, note, method, judged_at) \
         VALUES ('s', ?1, 'violation', 'written by the port', 'read the source', '2026-09-13T00:00:00+00:00')",
        [&site_id],
    )
    .expect("the port can write a verdict");
    drop(conn);

    let read_back = Command::new(&tool)
        .args(["status", "s", "--root", work.to_str().unwrap()])
        .env(ledger::SESSION_ENV_OVERRIDE, &session)
        .output()
        .expect("the tool runs");
    let body = String::from_utf8_lossy(&read_back.stdout);
    assert!(read_back.status.success(), "{}", String::from_utf8_lossy(&read_back.stderr));
    assert!(
        body.contains("\"judged\": 1"),
        "the Python tool did not see the verdict the port wrote: {body}"
    );

    std::fs::remove_dir_all(&work).ok();
    std::fs::remove_dir_all(std::env::temp_dir().join(ledger::TEMP_NAMESPACE).join(&session)).ok();
}
