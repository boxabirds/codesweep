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
