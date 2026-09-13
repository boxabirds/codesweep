//! The health check against two real index files.
//!
//! Both were produced by a real indexer during the reproduction. The pair is
//! the point: same source tree, one file moved outside the program, and an
//! index that is quietly short with nothing in it or around it saying so.

use std::path::{Path, PathBuf};

use resweep::resolution::{self, Health, Layer};

const TYPESCRIPT: &[&str] = &[".ts", ".tsx"];

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/scip")
        .join(name)
}

/// The source tree those indexes were made from, rebuilt so the check has
/// something to compare against. Four files, one of which the short index does
/// not contain.
struct Project(PathBuf);

impl Project {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("resweep-scip-{}-{label}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join("src")).expect("a directory");
        for name in ["app.ts", "document.ts", "report.ts", "session.ts"] {
            std::fs::write(dir.join("src").join(name), "export const a = 1;\n").expect("a file");
        }
        Self(dir)
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

#[test]
fn the_document_paths_can_be_read_out_of_a_real_index() {
    let docs = resolution::documents_in_scip(&fixture("healthy.scip"), TYPESCRIPT)
        .expect("the healthy index is readable");
    let mut names: Vec<&str> = docs.iter().map(String::as_str).collect();
    names.sort();
    assert_eq!(names, vec!["src/app.ts", "src/document.ts", "src/report.ts", "src/session.ts"]);
}

#[test]
fn the_short_index_is_missing_the_file_that_left_the_program() {
    let docs = resolution::documents_in_scip(
        &fixture("one-file-outside-the-program.scip"),
        TYPESCRIPT,
    )
    .expect("the short index is readable");
    let mut names: Vec<&str> = docs.iter().map(String::as_str).collect();
    names.sort();
    assert_eq!(names, vec!["src/app.ts", "src/document.ts", "src/session.ts"]);
    assert!(!names.contains(&"src/report.ts"), "the reproduction did not reproduce");
}

#[test]
fn a_healthy_index_over_its_own_project_is_complete() {
    let p = Project::new("healthy");
    let docs = resolution::documents_in_scip(&fixture("healthy.scip"), TYPESCRIPT)
        .expect("readable");
    let health = resolution::check(&p.0, &docs, TYPESCRIPT);
    assert_eq!(health, Health::Complete);
    assert_eq!(health.layer(), Layer::Resolved);
}

#[test]
fn the_short_index_is_caught_and_the_missing_file_is_named() {
    // The case the whole story rests on. The file is on disk, the index does
    // not contain it, and the indexer exited zero when it made that index.
    let p = Project::new("short");
    let docs = resolution::documents_in_scip(
        &fixture("one-file-outside-the-program.scip"),
        TYPESCRIPT,
    )
    .expect("readable");
    let health = resolution::check(&p.0, &docs, TYPESCRIPT);
    assert_eq!(health, Health::Partial { missing: vec!["src/report.ts".to_string()] });

    let answer = resolution::Answer::from(&health);
    assert_eq!(answer.layer, Layer::Matched);
    assert!(!answer.may_claim_exact(), "a short index produced an answer calling itself exact");
    assert_eq!(answer.gap.as_deref(), Some(&["src/report.ts".to_string()][..]));

    let refusal = resolution::refusal_for_exact_only(&health).expect("a rename must be refused");
    assert!(refusal.contains("src/report.ts"), "{refusal}");
}

#[test]
fn the_two_same_named_functions_are_separate_symbols_in_the_index() {
    // The reason the exact layer is worth building at all. A name-matching
    // layer returns both sets for either question; this does not.
    let bytes = std::fs::read(fixture("healthy.scip")).expect("readable");
    let text = String::from_utf8_lossy(&bytes);
    for symbol in ["src/`session.ts`/save().", "src/`document.ts`/save()."] {
        assert!(text.contains(symbol), "the index does not hold {symbol}");
    }
}

#[test]
fn the_check_does_not_care_what_the_indexer_said() {
    // There is nothing to care about. Both indexes were produced by runs that
    // printed done and exited zero, and one of them is short. The check takes
    // no exit status, no output and no diagnostics as arguments, which is
    // asserted here by the shape of the call rather than by inspection.
    let p = Project::new("silent");
    let short = resolution::documents_in_scip(
        &fixture("one-file-outside-the-program.scip"),
        TYPESCRIPT,
    )
    .expect("readable");
    assert!(!resolution::check(&p.0, &short, TYPESCRIPT).is_complete());
}
