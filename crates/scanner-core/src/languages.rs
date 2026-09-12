//! Language definitions and tree-sitter bindings.
//!
//! Defines supported languages with their file extensions and AST node types
//! for extracting symbols, imports, and other code entities.
//!
//! Design reference: Section 2.1

use serde::{Deserialize, Serialize};
use tree_sitter::Language as TsLanguage;

/// Supported programming languages for parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    Python,
    JavaScript,
    TypeScript,
    /// TypeScript with inline JSX. A separate variant rather than a flag on
    /// TypeScript, because it needs a different grammar: the plain grammar
    /// recovers from JSX rather than rejecting it, so a `.tsx` file parsed as
    /// TypeScript loses every call written inside markup and reports no error.
    Tsx,
    Go,
    Rust,
    Java,
}

impl Language {
    /// Create a Language from a file extension.
    ///
    /// Returns None for unsupported extensions.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "py" => Some(Self::Python),
            "js" | "mjs" | "cjs" | "jsx" => Some(Self::JavaScript),
            "ts" => Some(Self::TypeScript),
            "tsx" => Some(Self::Tsx),
            "go" => Some(Self::Go),
            "rs" => Some(Self::Rust),
            "java" => Some(Self::Java),
            _ => None,
        }
    }

    /// Get the language name as a string.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
            Self::Go => "go",
            Self::Rust => "rust",
            Self::Java => "java",
        }
    }

    /// Get the tree-sitter Language for parsing.
    pub fn tree_sitter_language(&self) -> TsLanguage {
        match self {
            Self::Python => tree_sitter_python::language(),
            Self::JavaScript => tree_sitter_javascript::language(),
            Self::TypeScript => tree_sitter_typescript::language_typescript(),
            Self::Tsx => tree_sitter_typescript::language_tsx(),
            Self::Go => tree_sitter_go::language(),
            Self::Rust => tree_sitter_rust::language(),
            Self::Java => tree_sitter_java::language(),
        }
    }

    /// Node types that represent class definitions.
    pub fn class_node_types(&self) -> &'static [&'static str] {
        match self {
            Self::Python => &["class_definition"],
            Self::JavaScript => &["class_declaration", "class"],
            Self::TypeScript | Self::Tsx => &["class_declaration", "class"],
            Self::Go => &["type_declaration"], // Go uses type declarations for struct types
            Self::Rust => &["struct_item", "enum_item", "trait_item"],
            Self::Java => &["class_declaration", "interface_declaration", "enum_declaration"],
        }
    }

    /// Node types that represent function/method definitions.
    pub fn function_node_types(&self) -> &'static [&'static str] {
        match self {
            Self::Python => &["function_definition"],
            Self::JavaScript => &[
                "function_declaration",
                "method_definition",
                "arrow_function",
                "function_expression",
            ],
            Self::TypeScript | Self::Tsx => &[
                "function_declaration",
                "method_definition",
                "arrow_function",
                "function_expression",
            ],
            Self::Go => &["function_declaration", "method_declaration"],
            Self::Rust => &["function_item"],
            Self::Java => &["method_declaration", "constructor_declaration"],
        }
    }

    /// Node types that represent import statements.
    pub fn import_node_types(&self) -> &'static [&'static str] {
        match self {
            Self::Python => &["import_statement", "import_from_statement"],
            Self::JavaScript => &["import_statement"],
            Self::TypeScript | Self::Tsx => &["import_statement"],
            Self::Go => &["import_declaration"],
            Self::Rust => &["use_declaration"],
            Self::Java => &["import_declaration"],
        }
    }

    /// Node types that represent decorators/attributes.
    pub fn decorator_node_types(&self) -> &'static [&'static str] {
        match self {
            Self::Python => &["decorator"],
            Self::JavaScript => &["decorator"],
            Self::TypeScript | Self::Tsx => &["decorator"],
            Self::Go => &[], // Go doesn't have decorators
            Self::Rust => &["attribute_item"],
            Self::Java => &["annotation", "marker_annotation"],
        }
    }

    /// Node types that represent string literals.
    pub fn string_node_types(&self) -> &'static [&'static str] {
        match self {
            Self::Python => &["string", "concatenated_string"],
            Self::JavaScript => &["string", "template_string"],
            Self::TypeScript | Self::Tsx => &["string", "template_string"],
            Self::Go => &["raw_string_literal", "interpreted_string_literal"],
            Self::Rust => &["string_literal", "raw_string_literal"],
            Self::Java => &["string_literal"],
        }
    }

    /// Node types that represent function/method calls.
    pub fn call_node_types(&self) -> &'static [&'static str] {
        match self {
            Self::Python => &["call"],
            Self::JavaScript => &["call_expression"],
            Self::TypeScript | Self::Tsx => &["call_expression"],
            Self::Go => &["call_expression"],
            Self::Rust => &["call_expression"],
            Self::Java => &["method_invocation"],
        }
    }

    /// Field name for getting superclass in class definitions.
    pub fn superclass_field(&self) -> Option<&'static str> {
        match self {
            Self::Python => Some("superclasses"),
            Self::JavaScript => Some("heritage"),
            Self::TypeScript | Self::Tsx => Some("heritage"),
            Self::Go => None, // Go uses embedding, not inheritance
            Self::Rust => None, // Rust uses traits, handled differently
            Self::Java => Some("superclass"),
        }
    }

    /// Node types that represent decision points for cyclomatic complexity.
    ///
    /// Each of these adds 1 to the cyclomatic complexity of a function.
    pub fn decision_point_types(&self) -> &'static [&'static str] {
        match self {
            Self::Python => &[
                "if_statement",
                "elif_clause",
                "for_statement",
                "while_statement",
                "except_clause",
                "with_statement",
                "assert_statement",
                "conditional_expression",  // ternary: x if cond else y
            ],
            Self::JavaScript | Self::TypeScript | Self::Tsx => &[
                "if_statement",
                "for_statement",
                "for_in_statement",
                "while_statement",
                "do_statement",
                "switch_case",
                "catch_clause",
                "ternary_expression",
            ],
            Self::Go => &[
                "if_statement",
                "for_statement",
                "expression_switch_statement",
                "type_switch_statement",
                "select_statement",
            ],
            Self::Rust => &[
                "if_expression",
                "for_expression",
                "while_expression",
                "loop_expression",
                "match_arm",
            ],
            Self::Java => &[
                "if_statement",
                "for_statement",
                "enhanced_for_statement",
                "while_statement",
                "do_statement",
                "switch_expression",
                "catch_clause",
                "ternary_expression",
            ],
        }
    }

    /// Node types that represent boolean operators that add to CC.
    ///
    /// Each && or || adds 1 to complexity (short-circuit evaluation paths).
    pub fn boolean_operator_node_types(&self) -> &'static [&'static str] {
        match self {
            Self::Python => &["boolean_operator"],
            Self::JavaScript | Self::TypeScript | Self::Tsx => &["binary_expression"],
            Self::Go => &["binary_expression"],
            Self::Rust => &["binary_expression"],
            Self::Java => &["binary_expression"],
        }
    }

    /// Boolean operator strings that indicate CC-relevant operators.
    pub fn boolean_operators(&self) -> &'static [&'static str] {
        match self {
            Self::Python => &["and", "or"],
            Self::JavaScript | Self::TypeScript | Self::Tsx => &["&&", "||"],
            Self::Go => &["&&", "||"],
            Self::Rust => &["&&", "||"],
            Self::Java => &["&&", "||"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_extension_python() {
        assert_eq!(Language::from_extension("py"), Some(Language::Python));
    }

    #[test]
    fn test_from_extension_javascript() {
        assert_eq!(Language::from_extension("js"), Some(Language::JavaScript));
        assert_eq!(Language::from_extension("jsx"), Some(Language::JavaScript));
        assert_eq!(Language::from_extension("mjs"), Some(Language::JavaScript));
    }

    /// TC-02 and TC-03. This test used to assert that `tsx` resolves to
    /// TypeScript, which is the defect rather than the requirement: the plain
    /// grammar recovers from JSX instead of rejecting it, so such a file loses
    /// every call written inside markup and reports nothing wrong. The
    /// assertion is changed rather than deleted, so the history of the fix
    /// stays visible in the test that encoded the bug.
    #[test]
    fn test_from_extension_typescript_and_tsx_are_separate() {
        assert_eq!(Language::from_extension("ts"), Some(Language::TypeScript));
        assert_eq!(Language::from_extension("tsx"), Some(Language::Tsx));
    }

    /// The two variants must resolve to different grammars and report
    /// different names. Sharing a match arm between them compiles, produces no
    /// error, and silently undoes the whole fix, which is exactly what
    /// happened once while making it.
    #[test]
    fn test_tsx_resolves_to_its_own_grammar_and_name() {
        assert_eq!(Language::Tsx.name(), "tsx");
        assert_eq!(Language::TypeScript.name(), "typescript");
        assert_ne!(
            Language::Tsx.tree_sitter_language(),
            Language::TypeScript.tree_sitter_language(),
            "Tsx resolved to the same grammar as TypeScript, so the variant \
             exists but changes nothing"
        );
    }

    /// TC-07. Extensions the crate has no reader for stay unmapped.
    ///
    /// Note for a later reader: `cts` and `mts` are TypeScript in every other
    /// tool, and are unmapped here. That is a real gap of the same class as
    /// the tsx defect, files silently contributing nothing, and it is
    /// deliberately left alone by this story, whose scope is markup files.
    /// This assertion pins the current behaviour so the gap is recorded rather
    /// than merely absent.
    #[test]
    fn test_extensions_without_a_reader() {
        assert_eq!(Language::from_extension("cts"), None);
        assert_eq!(Language::from_extension("mts"), None);
        assert_eq!(Language::from_extension("txt"), None);
    }

    #[test]
    fn test_from_extension_go() {
        assert_eq!(Language::from_extension("go"), Some(Language::Go));
    }

    #[test]
    fn test_from_extension_rust() {
        assert_eq!(Language::from_extension("rs"), Some(Language::Rust));
    }

    #[test]
    fn test_from_extension_java() {
        assert_eq!(Language::from_extension("java"), Some(Language::Java));
    }

    #[test]
    fn test_from_extension_unknown() {
        assert_eq!(Language::from_extension("txt"), None);
        assert_eq!(Language::from_extension("md"), None);
        assert_eq!(Language::from_extension(""), None);
    }

    #[test]
    fn test_language_name() {
        assert_eq!(Language::Python.name(), "python");
        assert_eq!(Language::JavaScript.name(), "javascript");
        assert_eq!(Language::TypeScript.name(), "typescript");
        assert_eq!(Language::Go.name(), "go");
        assert_eq!(Language::Rust.name(), "rust");
        assert_eq!(Language::Java.name(), "java");
    }

    #[test]
    fn test_class_node_types_non_empty() {
        assert!(!Language::Python.class_node_types().is_empty());
        assert!(!Language::JavaScript.class_node_types().is_empty());
        assert!(!Language::TypeScript.class_node_types().is_empty());
        assert!(!Language::Go.class_node_types().is_empty());
        assert!(!Language::Rust.class_node_types().is_empty());
        assert!(!Language::Java.class_node_types().is_empty());
    }

    #[test]
    fn test_function_node_types_non_empty() {
        assert!(!Language::Python.function_node_types().is_empty());
        assert!(!Language::JavaScript.function_node_types().is_empty());
        assert!(!Language::TypeScript.function_node_types().is_empty());
        assert!(!Language::Go.function_node_types().is_empty());
        assert!(!Language::Rust.function_node_types().is_empty());
        assert!(!Language::Java.function_node_types().is_empty());
    }

    #[test]
    fn test_import_node_types_non_empty() {
        assert!(!Language::Python.import_node_types().is_empty());
        assert!(!Language::JavaScript.import_node_types().is_empty());
        assert!(!Language::TypeScript.import_node_types().is_empty());
        assert!(!Language::Go.import_node_types().is_empty());
        assert!(!Language::Rust.import_node_types().is_empty());
        assert!(!Language::Java.import_node_types().is_empty());
    }

    #[test]
    fn test_decorator_node_types() {
        // Python, JS, TS, Rust, Java have decorators
        assert!(!Language::Python.decorator_node_types().is_empty());
        assert!(!Language::JavaScript.decorator_node_types().is_empty());
        assert!(!Language::TypeScript.decorator_node_types().is_empty());
        assert!(!Language::Rust.decorator_node_types().is_empty());
        assert!(!Language::Java.decorator_node_types().is_empty());
        // Go doesn't have decorators
        assert!(Language::Go.decorator_node_types().is_empty());
    }

    #[test]
    fn test_tree_sitter_language_loads() {
        // Just verify these don't panic
        let _ = Language::Python.tree_sitter_language();
        let _ = Language::JavaScript.tree_sitter_language();
        let _ = Language::TypeScript.tree_sitter_language();
        let _ = Language::Go.tree_sitter_language();
        let _ = Language::Rust.tree_sitter_language();
        let _ = Language::Java.tree_sitter_language();
    }
}
