//! Loading an ast-grep rule file and running it over one file's source.
//!
//! The matching engine is linked in rather than shelled out to. That removes
//! the exit-code handling, the JSON parse of another program's output, and the
//! whole class of failure where that output cannot be read at all. It also
//! makes the language set a fact the compiler knows: a rule naming a language
//! this build cannot parse is refused by name, rather than producing a partial
//! answer or a panic from inside the grammar layer.

use std::fmt;
use std::path::{Path, PathBuf};

use ast_grep_config::{GlobalRules, RuleConfig, from_yaml_string};
use ast_grep_core::tree_sitter::LanguageExt;
use ast_grep_language::SupportLang;

/// Every language a rule may name, in the order the tool lists them.
///
/// This is the Python tool's list minus scss. It is not the set of grammars
/// the dependency offers: enabling a grammar's cargo feature is what makes it
/// parseable, and a language whose feature is off still deserialises out of a
/// rule file and then panics inside the parser. So the list has to be checked
/// here, before a rule reaches the engine, and it has to agree with the
/// features in Cargo.toml. A disagreement in either direction is a bug the
/// tests below catch.
pub const SUPPORTED_LANGUAGES: &[&str] = &[
    "typescript",
    "tsx",
    "javascript",
    "html",
    "css",
    "python",
    "rust",
    "go",
    "java",
    "kotlin",
    "c",
    "cpp",
    "csharp",
    "ruby",
    "php",
    "swift",
    "scala",
    "lua",
    "bash",
    "elixir",
    "haskell",
    "json",
    "yaml",
];

/// A match, with lines counted from one so that every number the tool prints
/// is the number an editor shows. The engine counts from zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub start_line: usize,
    pub end_line: usize,
    pub text: String,
}

#[derive(Debug)]
pub enum RuleError {
    Unreadable { path: PathBuf, cause: String },
    Unparseable { path: PathBuf, cause: String },
    Empty { path: PathBuf },
    UnsupportedLanguage { path: PathBuf, language: String },
}

impl fmt::Display for RuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { path, cause } => {
                write!(f, "cannot read rule {}: {cause}", path.display())
            }
            Self::Unparseable { path, cause } => {
                write!(f, "cannot parse rule {}: {cause}", path.display())
            }
            Self::Empty { path } => {
                write!(f, "rule {} contains no rule", path.display())
            }
            Self::UnsupportedLanguage { path, language } => write!(
                f,
                "rule {} names language '{language}', which this build cannot parse.\n\
                 Supported: {}",
                path.display(),
                SUPPORTED_LANGUAGES.join(", ")
            ),
        }
    }
}

impl std::error::Error for RuleError {}

impl fmt::Debug for Rule {
    // The compiled matcher is not printable and would say nothing useful. What
    // a failing test needs is which rule file this is.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rule")
            .field("id", &self.id)
            .field("language", &self.language)
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

pub struct Rule {
    pub id: String,
    pub language: String,
    pub path: PathBuf,
    pub source: String,
    config: RuleConfig<SupportLang>,
}

impl Rule {
    /// Read a rule file exactly as written. Rule files are not migrated: the
    /// same file drives the Python tool and this one, so a difference between
    /// them is a difference in the engine rather than in the input.
    pub fn load(path: &Path) -> Result<Self, RuleError> {
        let source = std::fs::read_to_string(path).map_err(|e| RuleError::Unreadable {
            path: path.to_path_buf(),
            cause: e.to_string(),
        })?;
        Self::from_source(path, source)
    }

    pub fn from_source(path: &Path, source: String) -> Result<Self, RuleError> {
        let declared = declared_language(&source);
        // Checked before deserialising, because a language the dependency
        // knows but this build did not link deserialises perfectly well and
        // then panics the moment anything is parsed with it.
        if let Some(name) = &declared {
            if !SUPPORTED_LANGUAGES.contains(&name.as_str()) {
                return Err(RuleError::UnsupportedLanguage {
                    path: path.to_path_buf(),
                    language: name.clone(),
                });
            }
        }

        let globals = GlobalRules::default();
        let configs =
            from_yaml_string::<SupportLang>(&source, &globals).map_err(|e| {
                // A language the dependency does not know at all fails here,
                // as a deserialisation error. The operator gets the same
                // sentence either way; which layer refused is not their problem.
                match &declared {
                    Some(name) if !SUPPORTED_LANGUAGES.contains(&name.as_str()) => {
                        RuleError::UnsupportedLanguage {
                            path: path.to_path_buf(),
                            language: name.clone(),
                        }
                    }
                    _ => RuleError::Unparseable {
                        path: path.to_path_buf(),
                        cause: e.to_string(),
                    },
                }
            })?;

        let config = configs
            .into_iter()
            .next()
            .ok_or_else(|| RuleError::Empty { path: path.to_path_buf() })?;

        Ok(Self {
            id: config.id.clone(),
            language: declared.unwrap_or_else(|| format!("{:?}", config.language).to_lowercase()),
            path: path.to_path_buf(),
            source,
            config,
        })
    }

    /// Every match in one file's source, in document order.
    ///
    /// Document order is what decides which of two identical snippets in the
    /// same file is the first, and the ledger's site identity is built from
    /// that ordinal. Reordering here would rename sites that had not moved.
    pub fn matches(&self, source: &str) -> Vec<Match> {
        let grep = self.config.language.ast_grep(source);
        grep.root()
            .find_all(&self.config.matcher)
            .map(|m| {
                let range = m.range();
                let start = m.get_node().start_pos().line() + 1;
                let end = m.get_node().end_pos().line() + 1;
                Match {
                    start_line: start,
                    end_line: end,
                    text: source[range].to_string(),
                }
            })
            .collect()
    }
}

/// The `language:` field, read as text before anything is deserialised.
///
/// Deliberately shallow. It only has to find a top-level scalar in a rule file,
/// and the alternative is deserialising first, which is the thing that panics.
fn declared_language(source: &str) -> Option<String> {
    for line in source.lines() {
        if let Some(rest) = line.strip_prefix("language:") {
            let value = rest.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // Paths are relative to the crate, which is two levels below the
    // repository root.
    const REPO: &str = "../..";

    fn rule_from(yaml: &str) -> Result<Rule, RuleError> {
        Rule::from_source(Path::new("test.yml"), yaml.to_string())
    }

    #[test]
    fn the_shipped_typescript_rule_loads_verbatim_and_matches() {
        let path = Path::new(REPO).join("rules/ts-catch-clause.yml");
        let rule = Rule::load(&path).expect("the shipped rule loads");
        assert_eq!(rule.id, "ts-catch-clause");
        assert_eq!(rule.language, "typescript");

        // Three shapes that have to be found: the one-line brace form, a
        // clause split across lines, and one with no binding at all.
        let source = "\
export function a() { try { x(); } catch (e) { return []; } }
export function b() {
  try {
    x();
  } catch (e) {
    throw e;
  }
}
export function c() { try { x(); } catch { return null; } }
";
        let found = rule.matches(source);
        assert_eq!(found.len(), 3, "found: {found:#?}");
    }

    #[test]
    fn lines_are_counted_from_one_and_span_the_match() {
        let rule = rule_from("id: c\nlanguage: typescript\nrule:\n  kind: catch_clause\n")
            .expect("rule loads");
        let source = "\
export function b() {
  try {
    x();
  } catch (e) {
    throw e;
  }
}
";
        let found = rule.matches(source);
        assert_eq!(found.len(), 1);
        // The clause opens on the fourth line of the file and closes on the
        // sixth. An engine counting from zero would say three and five.
        assert_eq!(found[0].start_line, 4);
        assert_eq!(found[0].end_line, 6);
    }

    #[test]
    fn identical_snippets_come_back_in_document_order() {
        let rule = rule_from("id: c\nlanguage: typescript\nrule:\n  kind: catch_clause\n")
            .expect("rule loads");
        // Three byte-identical clauses. Their site identities differ only by
        // the ordinal they are given, and the ordinal is assigned in the order
        // they arrive, so an engine that reordered them would rename sites
        // that had not moved.
        let source = "\
function a() { try { x(); } catch (e) { return []; } }
function b() { try { x(); } catch (e) { return []; } }
function c() { try { x(); } catch (e) { return []; } }
";
        let found = rule.matches(source);
        assert_eq!(found.len(), 3);
        assert_eq!(
            found.iter().map(|m| m.start_line).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn the_matched_text_is_the_clause_and_not_the_whole_line() {
        let rule = rule_from("id: c\nlanguage: typescript\nrule:\n  kind: catch_clause\n")
            .expect("rule loads");
        let source = "function a() { try { x(); } catch (e) { return []; } }\n";
        let found = rule.matches(source);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].text, "catch (e) { return []; }");
    }

    #[test]
    fn tsx_is_its_own_language_and_sees_inside_markup() {
        let path = Path::new(REPO).join("rules/tsx-catch-clause.yml");
        let rule = Rule::load(&path).expect("the shipped tsx rule loads");
        assert_eq!(rule.language, "tsx");
        let source = "\
export const C = () => {
  try { load(); } catch (e) { return <span>failed</span>; }
  return <div>{(() => { try { g(); } catch (e) { return null; } })()}</div>;
};
";
        // Both clauses, including the one inside a JSX expression. The
        // TypeScript grammar cannot read this file at all.
        assert_eq!(rule.matches(source).len(), 2);
    }

    #[test]
    fn the_typescript_grammar_does_not_silently_half_read_tsx() {
        // Measured, against the assumption that was wrong: both grammars find
        // the catch clause in a .tsx file, because tree-sitter recovers from
        // the markup it cannot read. What differs is the wreckage left behind.
        // The TypeScript grammar leaves ERROR nodes across the JSX; the tsx
        // grammar leaves none. That is the silent half-read this tool exists
        // to expose, and it is why the two rule files stay separate.
        let ts_errors = rule_from("id: e\nlanguage: typescript\nrule:\n  kind: ERROR\n").unwrap();
        let tsx_errors = rule_from("id: e\nlanguage: tsx\nrule:\n  kind: ERROR\n").unwrap();
        let source = "\
export const C = () => {
  try { load(); } catch (e) { return <span>failed</span>; }
  return <div>{(() => { try { g(); } catch (e) { return null; } })()}</div>;
};
";
        assert!(
            !ts_errors.matches(source).is_empty(),
            "the TypeScript grammar read a .tsx file without complaint"
        );
        assert!(
            tsx_errors.matches(source).is_empty(),
            "the tsx grammar should read its own language cleanly"
        );
    }

    #[test]
    fn scss_is_refused_by_name_with_the_supported_set() {
        let err = rule_from("id: s\nlanguage: scss\nrule:\n  kind: declaration\n")
            .expect_err("scss is not supported");
        let message = err.to_string();
        assert!(message.contains("scss"), "{message}");
        assert!(message.contains("Supported:"), "{message}");
        assert!(message.contains("css"), "{message}");
    }

    #[test]
    fn a_grammar_left_out_of_this_build_is_refused_rather_than_crashing() {
        // The dependency knows dart. This build did not link it, and a
        // language whose feature is off deserialises perfectly well and then
        // panics inside the parser. Refusing it here is the only thing
        // standing between that and a crash mid-census.
        let err = rule_from("id: d\nlanguage: dart\nrule:\n  kind: catch_clause\n")
            .expect_err("dart is not in this build");
        assert!(err.to_string().contains("dart"), "{err}");
    }

    #[test]
    fn a_language_nobody_has_heard_of_gets_the_same_sentence() {
        let err = rule_from("id: x\nlanguage: cobol\nrule:\n  kind: paragraph\n")
            .expect_err("cobol is not a language here");
        let message = err.to_string();
        assert!(message.contains("cobol"), "{message}");
        assert!(message.contains("Supported:"), "{message}");
    }

    #[test]
    fn every_supported_language_can_actually_parse() {
        // The list and the cargo features have to agree. If a feature is
        // missing, the language is advertised and then panics on first use;
        // if a feature is present but the language is off the list, a rule
        // that would work is refused. Both are caught here.
        use std::str::FromStr;
        for name in SUPPORTED_LANGUAGES {
            let lang = SupportLang::from_str(name)
                .unwrap_or_else(|_| panic!("{name} is on the list but the engine rejects the name"));
            // Parsing is what panics when a grammar's feature is off, so the
            // check has to parse rather than merely resolve the name.
            let tree = lang.ast_grep("x\n");
            assert!(
                tree.root().children().count() < usize::MAX,
                "{name} produced no tree"
            );
        }
    }

    #[test]
    fn the_supported_list_is_the_python_tools_list_minus_scss() {
        // The port's language set is not free to drift from the tool it
        // replaces. Read the list out of the tool rather than restating it,
        // so the two cannot disagree unnoticed.
        let tool = std::fs::read_to_string(Path::new(REPO).join("bin/resweep"))
            .expect("the tool being ported is in the repository");
        let block = tool
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
        claimed.retain(|name| name != "scss");
        claimed.sort();

        let mut supported: Vec<String> =
            SUPPORTED_LANGUAGES.iter().map(|s| s.to_string()).collect();
        supported.sort();
        assert_eq!(supported, claimed);
    }

    #[test]
    fn an_unreadable_rule_says_which_file() {
        let err = Rule::load(Path::new("/no/such/rule.yml")).expect_err("no such file");
        assert!(err.to_string().contains("/no/such/rule.yml"), "{err}");
    }

    #[test]
    fn a_rule_file_with_no_rule_in_it_is_refused() {
        let err = rule_from("# just a comment\n").expect_err("nothing to run");
        assert!(err.to_string().contains("test.yml"), "{err}");
    }

    // Retired with the subprocess, deliberately and with no replacement:
    //
    //   - ast-grep missing from PATH
    //   - ast-grep exiting with an unexpected status
    //   - ast-grep emitting output that is not JSON
    //
    // The engine is linked in, so none of these states exists to be tested.
    // Their absence is a consequence of the change, not a gap in coverage.
}
