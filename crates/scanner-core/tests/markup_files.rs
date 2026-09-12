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