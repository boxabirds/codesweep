//! Behaviour of the reader on files that mix markup with code.
//!
//! These live outside `src` because they exercise only the public surface,
//! `Language::from_extension`, `parse_file` and `SymbolIndex`, and because
//! what they assert is behaviour rather than the internals the inline unit
//! tests cover.

use scanner_core::{parse_file, Language, ParseStats, SymbolIndex};
use std::io::Write;
use tempfile::NamedTempFile;

fn parse_code(code: &str, language: Language) -> (SymbolIndex, ParseStats) {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(code.as_bytes()).unwrap();

    let mut index = SymbolIndex::new();
    let path = file.path();
    let stats = parse_file(path, path, language, &mut index).unwrap();

    (index, stats)
}


// ---------------------------------------------------------------- tsx

/// Resolve the language the way a real caller does, from the file
/// extension, and only then parse. The defect under test lives in that
/// resolution, so a test handed a `Language` directly would never see it.
fn parse_by_extension(code: &str, ext: &str) -> (SymbolIndex, ParseStats) {
    let language = Language::from_extension(ext)
        .unwrap_or_else(|| panic!("no language is mapped to extension {ext:?}"));
    parse_code(code, language)
}

fn callees(index: &SymbolIndex) -> Vec<String> {
    index.call_sites().iter().map(|c| c.callee.clone()).collect()
}

/// A component file. Two of its calls, `formatLabel` and `computeSize`,
/// sit inside JSX expressions; the rest sit in ordinary statements.
///
/// That split is the whole point of the fixture. Measured against both
/// grammars, the plain TypeScript grammar finds every call outside the
/// markup and neither of the two inside it, while the tsx grammar finds
/// all of them. A fixture whose calls all sat outside the markup would
/// pass under both grammars and prove nothing.
const TSX_COMPONENT: &str = r##"
import { useState } from "react";
import { saveDraft } from "../api/drafts";
import { formatLabel } from "../util/labels";

export function SaveButton({ draftId, onSaved }) {
  const [busy, setBusy] = useState(false);

  async function handleClick() {
try {
  await saveDraft(draftId);
  onSaved(draftId);
} catch (err) {
  setBusy(false);
}
  }

  return (
<button className="save" disabled={busy} onClick={handleClick}>
  {formatLabel(busy)}
  <Spinner size={computeSize(busy)} />
</button>
  );
}

export function helperAfterJsx(x) {
  return trailingCall(x);
}
"##;

/// The same file with the markup replaced by an ordinary expression, and
/// the two calls kept where they were.
///
/// This is the control. Without it, a failure above could mean the fixture
/// is malformed rather than that the grammar is wrong, and a red result
/// would not distinguish the two.
const TS_CONTROL_WITHOUT_JSX: &str = r##"
import { useState } from "react";
import { saveDraft } from "../api/drafts";
import { formatLabel } from "../util/labels";

export function SaveButton({ draftId, onSaved }) {
  const [busy, setBusy] = useState(false);

  async function handleClick() {
try {
  await saveDraft(draftId);
  onSaved(draftId);
} catch (err) {
  setBusy(false);
}
  }

  return formatLabel(busy) + computeSize(busy);
}

export function helperAfterJsx(x) {
  return trailingCall(x);
}
"##;

/// TC-01. Calls written inside markup must be found.
///
/// They are not, today. `from_extension` maps `tsx` to the plain
/// TypeScript grammar, which cannot accept JSX. Tree-sitter recovers
/// rather than failing, so declarations either side of the markup are
/// still extracted and the file looks healthy: three symbols, three
/// imports, and four of its six calls. The two that are missing are the
/// two inside the markup, which in a component is where handlers and
/// rendered helpers are referenced.
///
/// That makes this the defect that matters most to the question "what
/// calls this". A caller in a component is exactly the caller a refactor
/// needs to find, and it is exactly the one that silently disappears.
#[test]
fn test_tsx_calls_inside_markup_are_found() {
    let (index, _stats) = parse_by_extension(TSX_COMPONENT, "tsx");
    let found = callees(&index);

    assert!(
        found.iter().any(|c| c == "formatLabel"),
        "a call inside a JSX expression was not recorded. found: {found:?}"
    );
    assert!(
        found.iter().any(|c| c == "computeSize"),
        "a call inside a nested JSX attribute was not recorded. found: {found:?}"
    );
}

/// TC-01, control. The identical calls outside markup are found today, so
/// a failure above is about the markup and not about the fixture.
#[test]
fn test_ts_control_without_jsx_finds_the_same_calls() {
    let (index, _stats) = parse_by_extension(TS_CONTROL_WITHOUT_JSX, "ts");
    let found = callees(&index);

    assert!(
        found.iter().any(|c| c == "formatLabel"),
        "the control fixture lost a call, so the fixture is wrong rather \
         than the grammar. found: {found:?}"
    );
    assert!(found.iter().any(|c| c == "computeSize"), "found: {found:?}");
}

/// TC-06. A file in another supported language is unaffected by any of
/// this. Present so that collateral damage from the fix has a case that
/// was passing before it.
#[test]
fn test_other_language_unaffected() {
    let (index, stats) = parse_by_extension("def f():\n    return g()\n", "py");
    assert_eq!(stats.symbols_found, 1);
    assert!(callees(&index).iter().any(|c| c == "g()" || c == "g"));
}
// ---------------------------------------------------------------- parse health

/// TC-14 and TC-01. Valid source of either flavour reports a clean read.
///
/// The markup file is the case that matters: before the grammar was routed it
/// reported error nodes while returning most of its content, which is the
/// shape of the original defect.
#[test]
fn test_valid_source_reports_no_error_nodes() {
    let (_index, tsx) = parse_by_extension(TSX_COMPONENT, "tsx");
    assert_eq!(
        tsx.error_nodes, 0,
        "a markup file parsed by the markup grammar should read cleanly"
    );

    let (_index, ts) = parse_by_extension(TS_CONTROL_WITHOUT_JSX, "ts");
    assert_eq!(ts.error_nodes, 0);
}

/// TC-05 and TC-15. Markup handed to the plain grammar is the case the count
/// exists for: most of the file survives, and only this number says so.
#[test]
fn test_markup_under_the_plain_grammar_is_reported_as_damaged() {
    let (index, stats) = parse_code(TSX_COMPONENT, Language::TypeScript);

    assert!(
        stats.error_nodes > 0,
        "the plain grammar cannot accept markup, so the tree must carry error nodes"
    );
    assert!(
        stats.symbols_found > 0,
        "the point of the count is that the file still looks productive: {} symbols",
        stats.symbols_found
    );
    assert!(
        !callees(&index).iter().any(|c| c == "formatLabel"),
        "this case is only meaningful while the plain grammar is still losing the markup calls"
    );
}

/// TC-08. An empty file is read cleanly and yields nothing. Emptiness must not
/// be reported as damage, or every empty file in a repository becomes a
/// warning nobody reads.
#[test]
fn test_empty_file_is_clean_and_empty() {
    let (index, stats) = parse_by_extension("", "tsx");

    assert_eq!(stats.error_nodes, 0);
    assert_eq!(stats.symbols_found, 0);
    assert!(index.symbols().is_empty());
}

/// TC-09. Text that is not source at all reports damage.
#[test]
fn test_non_source_text_reports_error_nodes() {
    let (_index, stats) = parse_by_extension("<<<< not source at all ][ }{", "tsx");

    assert!(stats.error_nodes > 0);
}

/// TC-29. A damaged read is still a success. Callers get their partial results
/// and the count beside them; the alternative, an Err, would throw away
/// everything the reader did manage on a half-refactored tree, which is the
/// tree this layer exists to work on.
#[test]
fn test_damaged_read_returns_ok() {
    let (index, stats) = parse_code(TSX_COMPONENT, Language::TypeScript);

    assert!(stats.error_nodes > 0);
    assert!(!index.symbols().is_empty(), "partial results are still returned");
}

/// TC-18. A path that does not exist is an error, and no count is invented.
#[test]
fn test_missing_file_is_an_io_error() {
    let mut index = SymbolIndex::new();
    let missing = std::path::Path::new("/nonexistent/definitely/not/here.tsx");
    let result = parse_file(missing, missing, Language::Tsx, &mut index);

    assert!(matches!(result, Err(scanner_core::ParseError::IoError(_))));
    assert!(index.symbols().is_empty());
}

// ------------------------------------------------- clean and degraded files

fn rel(index: &SymbolIndex) -> (Vec<String>, Vec<String>) {
    let mut clean: Vec<String> = index.files_parsed().iter().cloned().collect();
    let mut degraded: Vec<String> = index.files_degraded().iter().cloned().collect();
    clean.sort();
    degraded.sort();
    (clean, degraded)
}

fn parse_named(index: &mut SymbolIndex, name: &str, code: &str, language: Language) {
    let dir = std::env::temp_dir().join(format!("sc-{}-{}", std::process::id(), name));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, code).unwrap();
    let _ = parse_file(&path, std::path::Path::new(name), language, index);
}

/// TC-21 and TC-22. State before and after, not only the returned value.
#[test]
fn test_a_file_lands_in_exactly_one_set() {
    let mut index = SymbolIndex::new();
    let (clean, degraded) = rel(&index);
    assert!(clean.is_empty() && degraded.is_empty(), "before: in neither");

    parse_named(&mut index, "clean.tsx", TSX_COMPONENT, Language::Tsx);
    let (clean, degraded) = rel(&index);
    assert_eq!(clean, vec!["clean.tsx"]);
    assert!(degraded.is_empty());

    parse_named(&mut index, "broken.tsx", "<<<< ][ }{", Language::Tsx);
    let (clean, degraded) = rel(&index);
    assert_eq!(clean, vec!["clean.tsx"]);
    assert_eq!(degraded, vec!["broken.tsx"]);
}

/// TC-24 and TC-28. The disjointness invariant, and the case an implementation
/// that adds to one set without removing from the other will fail: re-reading
/// a file whose condition has changed.
#[test]
fn test_reindexing_moves_a_file_rather_than_listing_it_twice() {
    let mut index = SymbolIndex::new();

    parse_named(&mut index, "moves.tsx", "<<<< ][ }{", Language::Tsx);
    assert!(index.is_file_degraded("moves.tsx"));
    assert!(!index.is_file_parsed("moves.tsx"));

    parse_named(&mut index, "moves.tsx", TSX_COMPONENT, Language::Tsx);
    assert!(index.is_file_parsed("moves.tsx"));
    assert!(
        !index.is_file_degraded("moves.tsx"),
        "the file is in both sets, so every count derived from them is wrong"
    );

    let (clean, degraded) = rel(&index);
    assert_eq!(clean, vec!["moves.tsx"]);
    assert!(degraded.is_empty());
}

/// TC-23 and TC-25. A file that could not be read at all joins neither set,
/// and a rejected file never inflates the clean count.
#[test]
fn test_unreadable_file_joins_neither_set() {
    let mut index = SymbolIndex::new();
    let missing = std::path::Path::new("/nonexistent/definitely/not/here.tsx");
    let _ = parse_file(missing, std::path::Path::new("here.tsx"), Language::Tsx, &mut index);

    let (clean, degraded) = rel(&index);
    assert!(clean.is_empty(), "a file that was never read is not a clean read");
    assert!(degraded.is_empty());
    assert_eq!(index.stats().files_parsed, 0);
}

/// Workflow W-01, TC-13 and TC-26. A mixed tree: markup files with calls both
/// inside and outside their markup, a plain file, and one file that is not
/// source at all.
///
/// Counts are derived by reading the fixture, not copied from a run.
#[test]
fn test_mixed_tree_reports_every_call_and_does_not_abandon() {
    let mut index = SymbolIndex::new();

    parse_named(&mut index, "a.tsx", TSX_COMPONENT, Language::Tsx);
    parse_named(&mut index, "b.tsx", TSX_COMPONENT, Language::Tsx);
    parse_named(&mut index, "c.ts", TS_CONTROL_WITHOUT_JSX, Language::TypeScript);
    parse_named(&mut index, "d.tsx", "<<<< not source ][ }{", Language::Tsx);

    let (clean, degraded) = rel(&index);
    assert_eq!(clean, vec!["a.tsx", "b.tsx", "c.ts"], "three valid files");
    assert_eq!(degraded, vec!["d.tsx"], "exactly one unreadable as source");

    // The run did not abandon: statistics exist for all four.
    let stats = index.stats();
    assert_eq!(stats.files_parsed, 3);
    assert_eq!(stats.files_degraded, 1);

    // Each markup file holds two calls inside its markup, so both files
    // together contribute four of them.
    let found = callees(&index);
    assert_eq!(
        found.iter().filter(|c| *c == "formatLabel").count(),
        3,
        "two markup files plus the plain control: {found:?}"
    );
    assert_eq!(found.iter().filter(|c| *c == "computeSize").count(), 3);
}
