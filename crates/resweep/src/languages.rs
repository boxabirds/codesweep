//! Which file extensions each language actually covers.
//!
//! This exists for one reason: ast-grep treats `tsx` as a language separate
//! from `typescript`, so a rule with `language: typescript` never sees a .tsx
//! file. Without this map the tool would report full coverage of a scope it
//! had not read.
//!
//! scss is here and is not in the set of languages a rule may name. That is
//! deliberate and the two lists are answering different questions. A rule
//! cannot be written for scss, because no grammar for it is linked and the CSS
//! grammar reads it incompletely without saying so. But a .scss file sitting
//! in the scope is still source that nothing reached, and dropping it from
//! this map would stop the warning firing and make the tool silent about
//! exactly the files it cannot handle.

pub const LANGUAGE_EXTENSIONS: &[(&str, &[&str])] = &[
    ("typescript", &[".ts", ".cts", ".mts"]),
    ("tsx", &[".tsx"]),
    ("javascript", &[".js", ".cjs", ".mjs", ".jsx"]),
    ("html", &[".html", ".htm"]),
    ("css", &[".css"]),
    ("scss", &[".scss"]),
    ("python", &[".py", ".pyi"]),
    ("rust", &[".rs"]),
    ("go", &[".go"]),
    ("java", &[".java"]),
    ("kotlin", &[".kt", ".kts"]),
    ("c", &[".c", ".h"]),
    ("cpp", &[".cc", ".cpp", ".cxx", ".hpp", ".hh"]),
    ("csharp", &[".cs"]),
    ("ruby", &[".rb"]),
    ("php", &[".php"]),
    ("swift", &[".swift"]),
    ("scala", &[".scala"]),
    ("lua", &[".lua"]),
    ("bash", &[".sh", ".bash"]),
    ("elixir", &[".ex", ".exs"]),
    ("haskell", &[".hs"]),
    ("json", &[".json"]),
    ("yaml", &[".yml", ".yaml"]),
];

/// Languages this build recognises as source but for which no rule can be
/// written, because no grammar for them is linked.
///
/// The distinction matters to the operator and nowhere else. A file no rule
/// happens to cover is fixed by writing a rule. A file in one of these
/// languages cannot be, and telling someone to write a rule that cannot exist
/// wastes their time and, worse, teaches them to ignore the warning.
///
/// Dropping these from the extension map instead would make the tool silent
/// about files it cannot read, which is the failure this whole tool exists to
/// remove. They stay visible and are labelled honestly.
pub const UNPARSEABLE_LANGUAGES: &[&str] = &["scss"];

pub fn is_unparseable_extension(ext: &str) -> bool {
    UNPARSEABLE_LANGUAGES
        .iter()
        .filter_map(|name| extensions_for(name))
        .any(|exts| exts.contains(&ext))
}

/// Extensions that carry source worth auditing. Used only to decide which
/// uncovered extensions are worth reporting, so that a scope full of .md and
/// .lock files does not produce noise.
pub static SOURCE_EXTENSIONS: std::sync::LazyLock<Vec<&'static str>> =
    std::sync::LazyLock::new(|| {
        let mut all: Vec<&'static str> = LANGUAGE_EXTENSIONS
            .iter()
            .flat_map(|(_, exts)| exts.iter().copied())
            .collect();
        all.sort();
        all
    });

pub fn extensions_for(language: &str) -> Option<&'static [&'static str]> {
    LANGUAGE_EXTENSIONS
        .iter()
        .find(|(name, _)| *name == language)
        .map(|(_, exts)| *exts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scss_is_recognised_as_source_even_though_no_rule_can_name_it() {
        // The one that matters. A .scss file in the scope has to be reported
        // as unreachable, not quietly dropped from the denominator.
        assert!(SOURCE_EXTENSIONS.contains(&".scss"));
        assert!(!crate::rules::SUPPORTED_LANGUAGES.contains(&"scss"));
        assert!(is_unparseable_extension(".scss"));
    }

    #[test]
    fn a_language_a_rule_can_name_is_never_called_unparseable() {
        // The two lists are complements, and an overlap would put a language
        // in a warning saying no rule can name it while a rule names it.
        for name in crate::rules::SUPPORTED_LANGUAGES {
            assert!(
                !UNPARSEABLE_LANGUAGES.contains(name),
                "{name} is both offered and declared unparseable"
            );
            for ext in extensions_for(name).unwrap_or(&[]) {
                assert!(
                    !is_unparseable_extension(ext),
                    "{ext} belongs to {name}, which a rule can name"
                );
            }
        }
    }

    #[test]
    fn every_unparseable_language_is_still_recognised_as_source() {
        // Otherwise it would vanish from the denominator instead of being
        // reported, which is the outcome this list exists to prevent.
        for name in UNPARSEABLE_LANGUAGES {
            let exts = extensions_for(name)
                .unwrap_or_else(|| panic!("{name} is declared unparseable but has no extensions"));
            for ext in exts {
                assert!(SOURCE_EXTENSIONS.contains(ext), "{ext} is not counted as source");
            }
        }
    }

    #[test]
    fn tsx_is_a_language_of_its_own_with_its_own_extension() {
        assert_eq!(extensions_for("tsx"), Some(&[".tsx"][..]));
        assert!(!extensions_for("typescript").unwrap().contains(&".tsx"));
    }

    #[test]
    fn every_language_a_rule_may_name_is_in_the_extension_map() {
        // Otherwise a rule loads, runs, and its language is reported as absent
        // from the map in a warning that says the check could not run.
        for name in crate::rules::SUPPORTED_LANGUAGES {
            assert!(
                extensions_for(name).is_some(),
                "{name} can be named in a rule but has no extensions"
            );
        }
    }
}
