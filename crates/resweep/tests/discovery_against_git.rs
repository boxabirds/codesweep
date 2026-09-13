//! The new walker against the file list the current tool uses.
//!
//! The file set decides the denominator of every count the tool prints, so a
//! difference here is not a detail. This test does not assert the two agree.
//! It enumerates every disagreement and checks each one against a list of
//! differences that were looked at and accepted. A new difference fails,
//! whichever direction it goes in.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use resweep::discovery::{self, Method};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// What the current tool sees: tracked files, plus untracked files the ignore
/// rules do not exclude. Note the first half. A file that is tracked despite
/// being ignored is listed here and not by the walker, which is the headline
/// disagreement between the two.
fn git_list(scope: &Path) -> Option<BTreeSet<String>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(scope)
        .args(["ls-files", "--cached", "--others", "--exclude-standard"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect(),
    )
}

struct Disagreement {
    only_git: Vec<String>,
    only_walker: Vec<String>,
}

fn compare(scope: &Path) -> Option<Disagreement> {
    let git = git_list(scope)?;
    let walker: BTreeSet<String> = discovery::discover(scope).files.into_iter().collect();
    Some(Disagreement {
        only_git: git.difference(&walker).cloned().collect(),
        only_walker: walker.difference(&git).cloned().collect(),
    })
}

/// Differences that were looked at and accepted, with the reason. Anything
/// matching one of these is allowed; anything else fails.
///
/// 1. A file tracked in version control despite matching an ignore rule. The
///    old list includes it because it asks for tracked files; the walker
///    excludes it because it obeys the rule. Accepted: the rule is what the
///    repository wrote down, and ast-grep obeys it too, so the walker agrees
///    with the engine rather than with the file list.
/// 2. Anything under a directory in the never-walked set that version control
///    nonetheless tracks. Accepted for the same reason the set exists: a
///    vendored bundle in the denominator makes every percentage meaningless.
/// 3. A symlink. Version control lists it; the walker does not follow it.
///    Accepted after measuring: ast-grep does not follow it either, so
///    including it would put a file in the denominator that no census can
///    ever produce a site from.
/// 4. Anything inside a submodule. Version control lists the submodule as a
///    single entry and never its contents; the walker descends into it.
///    Accepted, and it fixes something: measured on a real monorepo, ast-grep
///    finds thirty-six sites across fourteen files inside a submodule that the
///    current tool's file list does not contain at all. The shipped tool
///    therefore counts sites in files its own extension check and surface
///    proposal cannot see. One walker for both removes that.
fn accepted(path: &str, scope: &Path) -> bool {
    if discovery::UNWALKED_DIRS
        .iter()
        .any(|d| path.split('/').any(|part| part == *d))
    {
        return true;
    }
    let full = scope.join(path);
    // A tracked file that the ignore rules exclude.
    if full.exists() {
        let ignored = Command::new("git")
            .arg("-C")
            .arg(scope)
            // --no-index on purpose. Without it, check-ignore refuses to call
            // a tracked file ignored, and a tracked-but-ignored file is
            // precisely the disagreement being classified.
            .args(["check-ignore", "--no-index", "-q", path])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ignored {
            return true;
        }
    }
    // A symlink. Listed by version control, followed by neither the walker
    // nor ast-grep.
    if full.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false) {
        return true;
    }
    // Inside a submodule: a directory on the way down carries its own .git.
    let mut here = full.parent();
    while let Some(dir) = here {
        if dir == scope {
            break;
        }
        if dir.join(".git").exists() {
            return true;
        }
        here = dir.parent();
    }
    // Listed by version control but not a file here at all.
    if !full.is_file() {
        return true;
    }
    false
}

fn assert_only_accepted(name: &str, scope: &Path) {
    let Some(d) = compare(scope) else {
        eprintln!("SKIP: {name} is not a git work tree, so there is nothing to compare against");
        return;
    };
    let unexplained_git: Vec<&String> = d
        .only_git
        .iter()
        .filter(|p| !accepted(p, scope))
        .collect();
    let unexplained_walker: Vec<&String> = d
        .only_walker
        .iter()
        .filter(|p| !accepted(p, scope))
        .collect();
    assert!(
        unexplained_git.is_empty() && unexplained_walker.is_empty(),
        "{name}: differences nobody has accounted for.\n\
         Only the old list has: {unexplained_git:#?}\n\
         Only the walker has: {unexplained_walker:#?}"
    );
    // A comparison that found nothing at all is not evidence of agreement.
    let total = d.only_git.len() + d.only_walker.len();
    eprintln!("{name}: {total} accounted-for differences");
}

#[test]
fn on_this_repository() {
    assert_only_accepted("this repository", &repo());
}

#[test]
fn on_the_pinned_third_party_fixture() {
    let cache = std::env::var("RESWEEP_FIXTURE_DIR").unwrap_or_else(|_| {
        format!("{}/.cache/github/axios/axios", std::env::var("HOME").unwrap_or_default())
    });
    let path = PathBuf::from(cache);
    if !path.join(".git").exists() {
        eprintln!("SKIP: no copy of the pinned fixture");
        return;
    }
    assert_only_accepted("the pinned fixture", &path);
}

#[test]
fn on_the_real_monorepo_when_it_is_present() {
    let repo = std::env::var("CEETRIX_REPO").unwrap_or_else(|_| {
        format!("{}/expts/claude-backlog", std::env::var("HOME").unwrap_or_default())
    });
    let path = PathBuf::from(repo);
    if !path.join(".git").exists() {
        eprintln!("SKIP: no monorepo checkout");
        return;
    }
    assert_only_accepted("the monorepo", &path);
}

#[test]
fn a_scope_with_no_ignore_files_says_so() {
    let dir = std::env::temp_dir().join(format!("resweep-plain-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("src")).expect("a directory");
    std::fs::write(dir.join("src/a.ts"), "x\n").expect("a file");
    let found = discovery::discover(&dir);
    assert_eq!(found.method, Method::Walk);
    assert_eq!(found.files, vec!["src/a.ts".to_string()]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_ignore_file_is_obeyed_and_reported() {
    let dir = std::env::temp_dir().join(format!("resweep-ignored-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("src")).expect("a directory");
    std::fs::write(dir.join(".gitignore"), "generated/\n*.log\n").expect("an ignore file");
    std::fs::create_dir_all(dir.join("generated")).expect("a directory");
    std::fs::write(dir.join("src/a.ts"), "x\n").expect("a file");
    std::fs::write(dir.join("generated/b.ts"), "x\n").expect("a generated file");
    std::fs::write(dir.join("noise.log"), "x\n").expect("a log");
    let found = discovery::discover(&dir);
    assert_eq!(found.method, Method::IgnoreFiles);
    assert_eq!(
        found.files,
        vec![".gitignore".to_string(), "src/a.ts".to_string()]
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_vendored_directory_stays_out_even_with_no_ignore_file() {
    let dir = std::env::temp_dir().join(format!("resweep-vendored-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("node_modules/pkg")).expect("a directory");
    std::fs::create_dir_all(dir.join("src")).expect("a directory");
    std::fs::write(dir.join("src/a.ts"), "x\n").expect("a file");
    std::fs::write(dir.join("node_modules/pkg/index.js"), "x\n").expect("a dependency");
    let found = discovery::discover(&dir);
    assert_eq!(found.files, vec!["src/a.ts".to_string()]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_relative_scope_resolves_against_the_root_and_not_the_working_directory() {
    let root = Path::new("/tmp/somewhere");
    assert_eq!(
        discovery::resolve_scope(root, Some("src")),
        PathBuf::from("/tmp/somewhere/src")
    );
    assert_eq!(
        discovery::resolve_scope(root, Some("/elsewhere/src")),
        PathBuf::from("/elsewhere/src")
    );
    assert_eq!(discovery::resolve_scope(root, None), root.to_path_buf());
}

/// A tree built for the purpose, so every expectation is a count of things
/// that were put there on purpose.
struct Tree(PathBuf);

impl Tree {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("resweep-tree-{}-{label}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("a directory");
        Self(dir)
    }
    fn file(&self, rel: &str, body: &str) -> &Self {
        let path = self.0.join(rel);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        std::fs::write(path, body).expect("a file");
        self
    }
    fn files(&self) -> Vec<String> {
        discovery::discover(&self.0).files
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

#[test]
fn a_vendored_directory_contributes_nothing() {
    let t = Tree::new("vendored");
    t.file("src/a.ts", "x\n")
        .file("node_modules/pkg/index.js", "x\n")
        .file("vendor/lib/thing.rb", "x\n");
    assert_eq!(t.files(), vec!["src/a.ts".to_string()]);
}

#[test]
fn a_build_output_directory_contributes_nothing() {
    let t = Tree::new("built");
    t.file("src/a.ts", "x\n")
        .file("dist/a.js", "x\n")
        .file("target/release/thing", "x\n")
        .file("build/out.o", "x\n");
    assert_eq!(t.files(), vec!["src/a.ts".to_string()]);
}

#[test]
fn a_file_matching_an_ignore_rule_contributes_nothing_and_one_that_does_not_contributes() {
    let t = Tree::new("ignored");
    t.file(".gitignore", "*.log\nsecret/\n")
        .file("src/a.ts", "x\n")
        .file("noise.log", "x\n")
        .file("secret/keys.ts", "x\n");
    // The ignore file itself is a tracked file in every repository that has
    // one, so it is in scope, and leaving it out would be its own drift.
    assert_eq!(t.files(), vec![".gitignore".to_string(), "src/a.ts".to_string()]);
}

#[test]
fn an_empty_scope_yields_nothing_and_does_not_crash() {
    let t = Tree::new("empty");
    assert!(t.files().is_empty());
}

#[test]
fn a_scope_with_exactly_one_file_yields_one() {
    let t = Tree::new("one");
    t.file("only.ts", "x\n");
    assert_eq!(t.files(), vec!["only.ts".to_string()]);
}

#[test]
fn the_exclusion_set_is_the_one_the_replaced_tool_hardcoded() {
    // A drift here means the two tools disagree about what is in scope while
    // both report confidently, which is the defect class this whole story
    // guards against. Read it out of the replaced tool rather than restating
    // it, and read that tool out of version control.
    const PYTHON_TOOL_COMMIT: &str = "80187f6";
    const PYTHON_TOOL_PATH: &str = "bin/codesweep";
    let out = Command::new("git")
        .arg("-C")
        .arg(repo())
        .arg("show")
        .arg(format!("{PYTHON_TOOL_COMMIT}:{PYTHON_TOOL_PATH}"))
        .output()
        .expect("git runs");
    if !out.status.success() {
        eprintln!("SKIP: the replaced tool is not reachable at {PYTHON_TOOL_COMMIT}");
        return;
    }
    let source = String::from_utf8_lossy(&out.stdout);
    let block = source
        .split("UNWALKED_DIRS = {")
        .nth(1)
        .and_then(|rest| rest.split('}').next())
        .expect("the tool declares an exclusion set");
    let mut hardcoded: Vec<String> = block
        .split(',')
        .map(|s| s.trim().trim_matches('"').trim_matches('\'').trim().to_string())
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .collect();
    hardcoded.sort();
    hardcoded.dedup();

    let mut ours: Vec<String> = discovery::UNWALKED_DIRS.iter().map(|s| s.to_string()).collect();
    ours.sort();
    assert_eq!(ours, hardcoded, "the exclusion sets have drifted apart");
}
