//! The ledger and the site identity, against the tool that was replaced.
//!
//! A test that only showed identities were stable would pass just as happily
//! against a drifted construction, including one recorded after drifting. So
//! every comparison here is against a value the Python tool produces on the
//! spot, and the Python tool is extracted from version control rather than
//! read from the working tree, which no longer contains it.
//!
//! The alternative was freezing the values it currently produces. That is
//! weaker for the reason above, and this is what version control is for.

use std::path::{Path, PathBuf};
use std::process::Command;

use resweep::ledger;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The last commit at which the Python implementation existed, and the path it
/// lived at then, which is the retired name: the rename came after this commit
/// and the port came after that. Named explicitly because the comparison is
/// against one specific historical artefact, and "before the port" stops being
/// a location the moment anything is rebased.
const PYTHON_TOOL_COMMIT: &str = "80187f6";
const PYTHON_TOOL_PATH: &str = "bin/codesweep";

/// Extract the Python tool from version control, once per process.
///
/// Written under a name of its own and then renamed into place. Tests in one
/// binary run in parallel threads, and a plain write leaves the file existing
/// but incomplete for as long as it takes to fill: another thread then sees it,
/// reads half a script, and fails for a reason that has nothing to do with what
/// it was testing. A rename is atomic, so the file is either absent or whole.
fn python_tool() -> Option<PathBuf> {
    let path = std::env::temp_dir().join(format!("resweep-python-{}", std::process::id()));
    if path.exists() {
        return Some(path);
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(repo())
        .arg("show")
        .arg(format!("{PYTHON_TOOL_COMMIT}:{PYTHON_TOOL_PATH}"))
        .output()
        .ok()?;
    if !out.status.success() || out.stdout.is_empty() {
        return None;
    }
    let staged = path.with_extension(format!("{:?}", std::thread::current().id()));
    std::fs::write(&staged, &out.stdout).ok()?;
    std::fs::rename(&staged, &path).ok()?;
    Some(path)
}

/// Run a snippet against the Python tool's own definitions, so the expected
/// value comes from the implementation rather than from a description of it.
fn python(snippet: &str) -> String {
    let Some(tool) = python_tool() else {
        panic!("the Python tool is not reachable at {PYTHON_TOOL_COMMIT}; \
                a shallow clone cannot run this comparison")
    };
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
    // Two constants are deliberately not compared. The temporary namespace and
    // the override variable both carry the tool's name, and the name changed
    // at the rename, which is the one commit between this artefact and the
    // port. tests/test_rename.sh is where that change is checked.
    assert_eq!(python("print(mod.SITE_ID_HEX_CHARS)"), ledger::SITE_ID_HEX_CHARS.to_string());
    assert_eq!(python("print(mod.SESSION_ENV)"), ledger::SESSION_ENV);
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
    // Someone mid-audit when they upgrade depends on this, and it is the
    // reason the schema version and the identity construction were held fixed.
    let session = format!("ledgerport-{}", std::process::id());
    // The artefact predates the rename, so it reads its own override variable
    // and writes under its own temporary namespace. Both are set, and the
    // ledger is carried across, so the comparison is about the ledger rather
    // than about where each tool keeps one.
    const RETIRED_OVERRIDE: &str = "CODESWEEP_SESSION_ID";
    const RETIRED_NAMESPACE: &str = "codesweep";
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

    // The Python tool, extracted from version control, because that is the
    // half of the round trip the working tree no longer has. It needs an
    // interpreter and a separately installed matching engine, neither of
    // which the port needs, so both absences are a skip rather than a pass.
    let Some(tool) = python_tool() else {
        eprintln!("SKIP: the Python tool is not reachable at {PYTHON_TOOL_COMMIT}");
        return;
    };
    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("SKIP: no python3, so the tool that wrote these ledgers cannot run");
        return;
    }
    if Command::new("ast-grep").arg("--version").output().is_err() {
        eprintln!("SKIP: no ast-grep, which the Python tool shells out to");
        return;
    }
    let ran = Command::new("python3")
        .arg(&tool)
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
        .env(RETIRED_OVERRIDE, &session)
        .output()
        .expect("the tool runs");
    assert!(ran.status.success(), "{}", String::from_utf8_lossy(&ran.stderr));

    // Built from the session name, not read from the environment: cases in
    // one binary share that environment and run in parallel.
    let written = std::env::temp_dir()
        .join(RETIRED_NAMESPACE)
        .join(&session)
        .join(format!("{}.db", ledger::repo_slug(&work)));
    assert!(written.exists(), "the Python tool wrote nothing at {written:?}");
    let path = std::env::temp_dir()
        .join(ledger::TEMP_NAMESPACE)
        .join(&session)
        .join(format!("{}.db", ledger::repo_slug(&work)));
    std::fs::create_dir_all(path.parent().expect("a session directory")).expect("a place to put it");
    std::fs::copy(&written, &path).expect("the ledger is carried across");

    let conn = ledger::connect_at(&path, false).expect("the ledger opens");
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

    std::fs::copy(&path, &written).expect("the ledger is carried back");
    let read_back = Command::new("python3")
        .arg(&tool)
        .args(["status", "s", "--root", work.to_str().unwrap()])
        .env(RETIRED_OVERRIDE, &session)
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
    std::fs::remove_dir_all(std::env::temp_dir().join(RETIRED_NAMESPACE).join(&session)).ok();
}

#[test]
fn the_supported_language_list_is_the_replaced_tools_list_minus_scss() {
    // The port's language set is not free to drift from the tool it replaces.
    // Read the list out of that tool rather than restating it, so the two
    // cannot disagree unnoticed, and read it from version control because the
    // working tree no longer holds it.
    let Some(tool) = python_tool() else {
        eprintln!("SKIP: the replaced tool is not reachable at {PYTHON_TOOL_COMMIT}");
        return;
    };
    let source = std::fs::read_to_string(&tool).expect("the extracted tool is readable");
    let block = source
        .split("LANGUAGE_EXTENSIONS = {")
        .nth(1)
        .and_then(|rest| rest.split("\n}").next())
        .expect("the tool declares a language map");
    let mut claimed: Vec<String> = block
        .lines()
        .filter_map(|line| line.trim().strip_prefix('"'))
        .filter_map(|line| line.split('"').next())
        .map(|s| s.to_string())
        .collect();
    assert!(claimed.contains(&"scss".to_string()), "the tool used to claim scss");
    claimed.retain(|name| name != "scss");
    claimed.sort();

    let mut supported: Vec<String> = resweep::rules::SUPPORTED_LANGUAGES
        .iter()
        .map(|s| s.to_string())
        .collect();
    supported.sort();
    assert_eq!(supported, claimed);

    // scss is still recognised as source, so a .scss file in a scope is still
    // reported as unreachable rather than dropped from the denominator.
    assert!(resweep::languages::SOURCE_EXTENSIONS.contains(&".scss"));
}
