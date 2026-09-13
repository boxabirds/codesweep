//! Whether an index of a project is complete with respect to the project.
//!
//! The exact layer answers questions by resolving which declaration a name
//! refers to, which is the only way to tell two same-named things apart. It
//! needs a project that resolves, and when a project does not fully resolve it
//! does not refuse: it produces a result smaller than the truth that looks
//! exactly like a complete one.
//!
//! That was reproduced on purpose before any of this was written, and the
//! reproduction is what decides the shape here. Uninstalling a dependency
//! changed nothing. Importing a name from a module that does not exist changed
//! nothing that mattered. What produced a short index was a file left on disk
//! and outside the program: one symbol's references fell from eight to six,
//! the documents from four to three, and the indexer printed `done` and exited
//! zero.
//!
//! So the check cannot ask the indexer how it went. It compares what the index
//! contains against what is on disk.

use std::collections::BTreeSet;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

use crate::discovery;

/// How an answer was arrived at. Two values and no third: there is no
/// partial-exact, and an answer built from a short index is never called
/// resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// Each name was resolved to the declaration it refers to.
    Resolved,
    /// Names were matched as text. Honest, useful, and not exact.
    Matched,
}

impl Layer {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Resolved => "resolved",
            Self::Matched => "matched",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Health {
    /// Every source file the walker found is in the index.
    Complete,
    /// Source files are on disk and absent from the index. Named, not just
    /// counted: the count says how bad it is and the names say where, and only
    /// the names let anyone fix it.
    Partial { missing: Vec<String> },
    /// The artefact could not be read at all. Distinct from partial, because an
    /// unreadable index and an incomplete one need different remedies.
    Unreadable { reason: String },
}

impl Health {
    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Complete)
    }

    /// The layer an answer derived from this index may claim.
    pub fn layer(&self) -> Layer {
        if self.is_complete() { Layer::Resolved } else { Layer::Matched }
    }
}

/// Compare an index against the files on disk under the same root.
///
/// `indexed` is the set of document paths the index contains, relative to the
/// root. `extensions` limits both sides to the language the index is of: an
/// index of TypeScript is not incomplete for containing no stylesheets.
///
/// The indexer's exit status, its output and the presence of any error are
/// deliberately not consulted. All three reported success while the index was
/// short, which is the entire reason this function exists. That sentence is
/// here because it is the kind of remark a later reader deletes as redundant.
pub fn check(root: &Path, indexed: &[String], extensions: &[&str]) -> Health {
    let on_disk: BTreeSet<String> = discovery::discover(root)
        .files
        .into_iter()
        .filter(|f| has_extension(f, extensions))
        .collect();
    let in_index: BTreeSet<String> = indexed
        .iter()
        .filter(|f| has_extension(f, extensions))
        .cloned()
        .collect();

    let missing: Vec<String> = on_disk.difference(&in_index).cloned().collect();
    if missing.is_empty() {
        Health::Complete
    } else {
        Health::Partial { missing }
    }
}

fn has_extension(path: &str, extensions: &[&str]) -> bool {
    let ext = Path::new(path)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    extensions.contains(&ext.as_str())
}

/// The document paths a SCIP index contains, relative to its root.
///
/// Read from the artefact itself. The paths are length-delimited strings in the
/// encoding, and every document carries one, so they can be recovered without a
/// protocol buffer library. That is a deliberate trade: a parser for the whole
/// format would be a dependency and a maintenance burden for one field, and the
/// field is the only thing this check needs.
pub fn documents_in_scip(index: &Path, extensions: &[&str]) -> Result<Vec<String>, String> {
    let bytes = std::fs::read(index).map_err(|e| format!("cannot read {}: {e}", index.display()))?;
    if bytes.is_empty() {
        return Err(format!("{} is empty", index.display()));
    }
    let mut found = BTreeSet::new();
    let text = String::from_utf8_lossy(&bytes);
    let mut current = String::new();
    for ch in text.chars() {
        // Paths are printable and contain no whitespace; anything else ends a
        // candidate. Collecting greedily and filtering by extension afterwards
        // is what keeps this from needing to understand the framing.
        if ch.is_ascii_graphic() && ch != '"' {
            current.push(ch);
        } else {
            if has_extension(&current, extensions) {
                found.insert(trim_to_path(&current, extensions));
            }
            current.clear();
        }
    }
    if has_extension(&current, extensions) {
        found.insert(trim_to_path(&current, extensions));
    }

    // The same document is recoverable under two spellings: its path relative
    // to the root, and a bare filename inside a symbol string. Keeping both
    // would put entries in the index set that no walked path can ever equal,
    // and one of them could accidentally equal a file at the root and hide a
    // real gap. Drop any candidate that is the tail of another.
    let all: Vec<String> = found.into_iter().collect();
    let fullest: Vec<String> = all
        .iter()
        .filter(|short| {
            !all.iter().any(|long| {
                long.len() > short.len() && long.ends_with(&format!("/{short}"))
            })
        })
        .cloned()
        .collect();
    Ok(fullest)
}

/// A recovered string may carry framing bytes in front of the path. Keep from
/// the last plausible path start to the end of the extension.
fn trim_to_path(candidate: &str, extensions: &[&str]) -> String {
    for ext in extensions {
        if let Some(end) = candidate.rfind(ext) {
            let end = end + ext.len();
            let head = &candidate[..end];
            let start = head
                .char_indices()
                .rev()
                .take_while(|(_, c)| c.is_ascii_alphanumeric() || "._-/".contains(*c))
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            return head[start..].to_string();
        }
    }
    candidate.to_string()
}

/// What an answer must carry, whichever layer produced it.
#[derive(Debug)]
pub struct Answer {
    pub layer: Layer,
    /// Present whenever the layer is not resolved and the reason is a short
    /// index rather than an absent one.
    pub gap: Option<Vec<String>>,
    /// Why the approximate layer answered, when the exact one was asked for.
    pub substituted_because: Option<String>,
}

impl Answer {
    pub fn from(health: &Health) -> Self {
        match health {
            Health::Complete => Self { layer: Layer::Resolved, gap: None, substituted_because: None },
            Health::Partial { missing } => Self {
                layer: Layer::Matched,
                gap: Some(missing.clone()),
                substituted_because: Some(format!(
                    "{} source file(s) are on disk and absent from the index, so an exact \
                     answer would be smaller than the truth without saying so",
                    missing.len()
                )),
            },
            Health::Unreadable { reason } => Self {
                layer: Layer::Matched,
                gap: None,
                substituted_because: Some(format!("the index could not be read: {reason}")),
            },
        }
    }

    /// Whether this answer may use the words exact or complete. It may not,
    /// unless every name in it was resolved.
    pub fn may_claim_exact(&self) -> bool {
        self.layer == Layer::Resolved
    }
}

/// A question that cannot be answered by matching names.
///
/// A rename is the clearest. Acting on a name-matched set does not misinform,
/// it corrupts: every same-named thing that was not the target gets changed, in
/// one commit, across the tree.
pub fn refusal_for_exact_only(health: &Health) -> Option<String> {
    if health.is_complete() {
        return None;
    }
    Some(match health {
        Health::Partial { missing } => format!(
            "This question can only be answered by resolving names, and the index is \
             missing {} source file(s) that are on disk: {}.\n\
             Answering it by matching names would change every same-named thing that \
             was not the target. Index the whole project and ask again.",
            missing.len(),
            missing.join(", ")
        ),
        Health::Unreadable { reason } => format!(
            "This question can only be answered by resolving names, and the index could \
             not be read: {reason}.\n\
             Build an index of the project and ask again."
        ),
        Health::Complete => unreachable!("handled above"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TS: &[&str] = &[".ts", ".tsx"];

    struct Tree(PathBuf);

    impl Tree {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("resweep-res-{}-{label}", std::process::id()));
            std::fs::remove_dir_all(&dir).ok();
            std::fs::create_dir_all(dir.join("src")).expect("a directory");
            Self(dir)
        }
        fn file(&self, rel: &str) -> &Self {
            let p = self.0.join(rel);
            std::fs::create_dir_all(p.parent().expect("a parent")).expect("a directory");
            std::fs::write(p, "export const a = 1;\n").expect("a file");
            self
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    #[test]
    fn an_index_holding_every_file_is_complete() {
        let t = Tree::new("complete");
        t.file("src/a.ts").file("src/b.ts");
        let health = check(&t.0, &["src/a.ts".into(), "src/b.ts".into()], TS);
        assert_eq!(health, Health::Complete);
        assert_eq!(health.layer(), Layer::Resolved);
    }

    #[test]
    fn a_file_on_disk_and_missing_from_the_index_is_named() {
        // The reproduction, as a unit. A count alone says how bad it is; the
        // name is what lets anyone fix it.
        let t = Tree::new("short");
        t.file("src/a.ts").file("src/b.ts").file("src/report.ts");
        let health = check(&t.0, &["src/a.ts".into(), "src/b.ts".into()], TS);
        assert_eq!(health, Health::Partial { missing: vec!["src/report.ts".to_string()] });
        assert_eq!(health.layer(), Layer::Matched);
        assert!(!Answer::from(&health).may_claim_exact());
    }

    #[test]
    fn an_index_of_nothing_is_partial_and_not_complete_over_an_empty_set() {
        // The case that catches a check written as whether the two sets are
        // equal, which is true of two empty ones.
        let t = Tree::new("empty-index");
        t.file("src/a.ts").file("src/b.ts");
        let health = check(&t.0, &[], TS);
        match health {
            Health::Partial { missing } => assert_eq!(missing.len(), 2),
            other => panic!("an index of nothing was reported as {other:?}"),
        }
    }

    #[test]
    fn a_project_with_no_files_of_that_language_is_complete() {
        // Nothing is missing, so nothing is wrong. An index of TypeScript is
        // not incomplete for containing no stylesheets.
        let t = Tree::new("other-language");
        t.file("src/a.css").file("src/b.md");
        assert_eq!(check(&t.0, &[], TS), Health::Complete);
    }

    #[test]
    fn a_file_the_repository_ignores_is_not_a_gap() {
        let t = Tree::new("ignored");
        t.file("src/a.ts");
        std::fs::write(t.0.join(".gitignore"), "generated/\n").expect("an ignore file");
        std::fs::create_dir_all(t.0.join("generated")).expect("a directory");
        std::fs::write(t.0.join("generated/b.ts"), "export const b = 2;\n").expect("a file");
        assert_eq!(check(&t.0, &["src/a.ts".into()], TS), Health::Complete);
    }

    #[test]
    fn an_unreadable_index_is_not_reported_as_partial() {
        let err = documents_in_scip(Path::new("/no/such/index.scip"), TS)
            .expect_err("there is no such file");
        assert!(err.contains("/no/such/index.scip"), "{err}");
        let health = Health::Unreadable { reason: err };
        assert!(!health.is_complete());
        let answer = Answer::from(&health);
        assert!(!answer.may_claim_exact());
        assert!(answer.gap.is_none(), "an unreadable index has no list of missing files");
        assert!(answer.substituted_because.is_some());
    }

    #[test]
    fn an_answer_from_a_short_index_carries_the_gap_and_never_claims_exactness() {
        let health = Health::Partial { missing: vec!["src/report.ts".to_string()] };
        let answer = Answer::from(&health);
        assert_eq!(answer.layer, Layer::Matched);
        assert_eq!(answer.gap.as_deref(), Some(&["src/report.ts".to_string()][..]));
        let why = answer.substituted_because.expect("a reason");
        assert!(why.contains("smaller than the truth"), "{why}");
    }

    #[test]
    fn an_exact_only_question_is_refused_and_says_what_would_answer_it() {
        let health = Health::Partial { missing: vec!["src/report.ts".to_string()] };
        let refusal = refusal_for_exact_only(&health).expect("a refusal");
        assert!(refusal.contains("src/report.ts"), "{refusal}");
        assert!(refusal.contains("ask again"), "the refusal offers no way forward: {refusal}");
        assert!(refusal_for_exact_only(&Health::Complete).is_none());
    }

    #[test]
    fn the_two_layers_are_named_and_there_is_no_third() {
        assert_eq!(Layer::Resolved.as_str(), "resolved");
        assert_eq!(Layer::Matched.as_str(), "matched");
        // A partial-exact would be the failure this whole module prevents.
        for health in [
            Health::Complete,
            Health::Partial { missing: vec!["x.ts".into()] },
            Health::Unreadable { reason: "x".into() },
        ] {
            let layer = health.layer();
            assert!(layer == Layer::Resolved || layer == Layer::Matched);
        }
    }
}
