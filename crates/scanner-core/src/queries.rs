//! Query implementations for SymbolIndex.
//!
//! Provides pattern-matching queries over the indexed data with
//! performance targets of < 50ms p99 per query.
//!
//! Design reference: Section 3.1

use std::fs;

use crate::index::{CallSite, Import, StringLiteral, Symbol, SymbolIndex, SymbolKind};

/// Check if a string matches a glob-style pattern.
///
/// Supports `*` as a wildcard matching zero or more characters.
fn glob_match(pattern: &str, text: &str) -> bool {
    // Fast path: exact match
    if !pattern.contains('*') {
        return pattern == text;
    }

    // Split pattern by wildcards
    let parts: Vec<&str> = pattern.split('*').collect();

    // Empty pattern with just wildcards matches everything
    if parts.iter().all(|p| p.is_empty()) {
        return true;
    }

    let mut pos = 0;

    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }

        // First part must be at start if pattern doesn't start with *
        if i == 0 && !pattern.starts_with('*') {
            if !text.starts_with(part) {
                return false;
            }
            pos = part.len();
            continue;
        }

        // Last part must be at end if pattern doesn't end with *
        if i == parts.len() - 1 && !pattern.ends_with('*') {
            if !text.ends_with(part) {
                return false;
            }
            continue;
        }

        // Find the part in the remaining text
        if let Some(found_pos) = text[pos..].find(part) {
            pos += found_pos + part.len();
        } else {
            return false;
        }
    }

    true
}

impl SymbolIndex {
    /// Query symbols by name pattern and optional kind filter.
    ///
    /// Pattern supports glob-style wildcards (`*`).
    pub fn query_symbols(&self, pattern: &str, kind: Option<SymbolKind>) -> Vec<&Symbol> {
        // Fast path: exact match with index lookup
        if !pattern.contains('*') {
            let indices = self.symbols_by_name(pattern);
            return indices
                .iter()
                .filter_map(|&idx| self.get_symbol(idx))
                .filter(|sym| kind.is_none_or(|k| sym.kind == k))
                .collect();
        }

        // Slow path: scan all symbols
        self.symbols()
            .iter()
            .filter(|sym| {
                glob_match(pattern, &sym.name) && kind.is_none_or(|k| sym.kind == k)
            })
            .collect()
    }

    /// Query subclasses of a base class.
    ///
    /// Returns symbols that inherit from the specified class.
    pub fn query_subclasses(&self, base_class: &str) -> Vec<&Symbol> {
        let inheritance_indices = self.subclass_indices(base_class);

        inheritance_indices
            .iter()
            .filter_map(|&idx| self.get_inheritance(idx))
            .filter_map(|inh| {
                // Find the symbol for the child class
                let child_indices = self.symbols_by_name(&inh.child_class);
                child_indices
                    .iter()
                    .filter_map(|&idx| self.get_symbol(idx))
                    .find(|sym| sym.kind == SymbolKind::Class)
            })
            .collect()
    }

    /// Query call sites where a function is called.
    ///
    /// Pattern supports glob-style wildcards.
    pub fn query_callsites(&self, function_name: &str) -> Vec<&CallSite> {
        self.call_sites()
            .iter()
            .filter(|cs| glob_match(function_name, &cs.callee))
            .collect()
    }

    /// Query imports matching a module pattern.
    ///
    /// Pattern supports glob-style wildcards.
    pub fn query_imports(&self, module_pattern: &str) -> Vec<&Import> {
        self.imports()
            .iter()
            .filter(|imp| glob_match(module_pattern, &imp.module_path))
            .collect()
    }

    /// Query string literals with optional filters.
    ///
    /// - `min_length`: Minimum string length
    /// - `contains`: String must contain this substring
    /// - `file_pattern`: File path must match this glob pattern
    pub fn query_strings(
        &self,
        min_length: Option<usize>,
        contains: Option<&str>,
        file_pattern: Option<&str>,
    ) -> Vec<&StringLiteral> {
        self.strings()
            .iter()
            .filter(|sl| {
                // Filter by minimum length
                if let Some(min) = min_length {
                    if sl.content.len() < min {
                        return false;
                    }
                }

                // Filter by contains
                if let Some(needle) = contains {
                    if !sl.content.contains(needle) {
                        return false;
                    }
                }

                // Filter by file pattern
                if let Some(pattern) = file_pattern {
                    if !glob_match(pattern, &sl.location.file) {
                        return false;
                    }
                }

                true
            })
            .collect()
    }

    /// Query files matching a pattern.
    ///
    /// Returns unique file paths from the index.
    pub fn query_files(&self, pattern: &str) -> Vec<&str> {
        self.files_parsed()
            .iter()
            .filter(|file| glob_match(pattern, file))
            .map(String::as_str)
            .collect()
    }

    /// Query symbols with decorators/attributes matching a pattern.
    ///
    /// Pattern supports glob-style wildcards.
    pub fn query_decorated(&self, decorator_pattern: &str) -> Vec<&Symbol> {
        self.symbols()
            .iter()
            .filter(|sym| {
                sym.decorators
                    .iter()
                    .any(|dec| glob_match(decorator_pattern, dec))
            })
            .collect()
    }
}

/// Read a section of a file from disk.
///
/// Returns lines from `start_line` to `end_line` (1-indexed, inclusive).
/// Returns empty string if file doesn't exist or range is invalid.
pub fn read_file_section(file_path: &str, start_line: u32, end_line: u32) -> String {
    let content = match fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };

    let lines: Vec<&str> = content.lines().collect();

    // Convert to 0-indexed
    let start = (start_line.saturating_sub(1)) as usize;
    let end = end_line as usize;

    if start >= lines.len() {
        return String::new();
    }

    let end = end.min(lines.len());

    lines[start..end].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::Location;
    use crate::languages::Language;

    fn sample_location(file: &str) -> Location {
        Location::new(file, 1, 5, 0)
    }

    fn build_test_index() -> SymbolIndex {
        let mut index = SymbolIndex::new();

        // Add some symbols
        index.add_symbol(Symbol::new(
            "MyClass",
            SymbolKind::Class,
            sample_location("src/models.py"),
            Language::Python,
        ));
        index.add_symbol(Symbol::new(
            "my_function",
            SymbolKind::Function,
            sample_location("src/utils.py"),
            Language::Python,
        ));
        index.add_symbol(Symbol::new(
            "my_method",
            SymbolKind::Method,
            sample_location("src/models.py"),
            Language::Python,
        ).with_parent("MyClass").with_decorators(vec!["staticmethod".to_string()]));
        index.add_symbol(Symbol::new(
            "ChildClass",
            SymbolKind::Class,
            sample_location("src/models.py"),
            Language::Python,
        ));
        index.add_symbol(Symbol::new(
            "AnotherChild",
            SymbolKind::Class,
            sample_location("src/other.py"),
            Language::Python,
        ));

        // Add inheritance
        use crate::index::Inheritance;
        index.add_inheritance(Inheritance::new(
            "ChildClass",
            "MyClass",
            sample_location("src/models.py"),
        ));
        index.add_inheritance(Inheritance::new(
            "AnotherChild",
            "MyClass",
            sample_location("src/other.py"),
        ));

        // Add imports
        use crate::index::Import;
        index.add_import(Import::new("os.path", sample_location("src/utils.py")));
        index.add_import(Import::new("collections", sample_location("src/utils.py")));
        index.add_import(Import::new(
            "django.db.models",
            sample_location("src/models.py"),
        ));

        // Add call sites
        use crate::index::CallSite;
        index.add_call_site(
            CallSite::new("print", sample_location("src/utils.py"))
                .with_enclosing_function("my_function"),
        );
        index.add_call_site(CallSite::new("open", sample_location("src/utils.py")));
        index.add_call_site(CallSite::new("save", sample_location("src/models.py")));

        // Add strings
        use crate::index::StringLiteral;
        index.add_string(StringLiteral::new(
            "Hello, World!",
            sample_location("src/main.py"),
        ));
        index.add_string(StringLiteral::new(
            "config.yaml",
            sample_location("src/config.py"),
        ));
        index.add_string(StringLiteral::new("ab", sample_location("src/short.py")));

        // Mark files as parsed
        index.mark_file_parsed("src/models.py");
        index.mark_file_parsed("src/utils.py");
        index.mark_file_parsed("src/main.py");
        index.mark_file_parsed("src/config.py");
        index.mark_file_parsed("src/other.py");
        index.mark_file_parsed("src/short.py");

        index
    }

    #[test]
    fn test_glob_match_exact() {
        assert!(glob_match("hello", "hello"));
        assert!(!glob_match("hello", "world"));
    }

    #[test]
    fn test_glob_match_wildcard_suffix() {
        assert!(glob_match("hello*", "helloworld"));
        assert!(glob_match("hello*", "hello"));
        assert!(!glob_match("hello*", "hell"));
    }

    #[test]
    fn test_glob_match_wildcard_prefix() {
        assert!(glob_match("*world", "helloworld"));
        assert!(glob_match("*world", "world"));
        assert!(!glob_match("*world", "worldx"));
    }

    #[test]
    fn test_glob_match_wildcard_middle() {
        assert!(glob_match("hello*world", "helloworld"));
        assert!(glob_match("hello*world", "hello_big_world"));
        assert!(!glob_match("hello*world", "helloworl"));
    }

    #[test]
    fn test_glob_match_only_wildcard() {
        assert!(glob_match("*", "anything"));
        assert!(glob_match("*", ""));
    }

    #[test]
    fn test_query_symbols_exact() {
        let index = build_test_index();
        let results = index.query_symbols("MyClass", None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "MyClass");
    }

    #[test]
    fn test_query_symbols_pattern() {
        let index = build_test_index();
        let results = index.query_symbols("my_*", None);
        assert_eq!(results.len(), 2); // my_function, my_method
    }

    #[test]
    fn test_query_symbols_with_kind() {
        let index = build_test_index();
        let results = index.query_symbols("*", Some(SymbolKind::Class));
        assert_eq!(results.len(), 3); // MyClass, ChildClass, AnotherChild
    }

    #[test]
    fn test_query_subclasses() {
        let index = build_test_index();
        let results = index.query_subclasses("MyClass");
        assert_eq!(results.len(), 2); // ChildClass, AnotherChild
    }

    #[test]
    fn test_query_callsites_exact() {
        let index = build_test_index();
        let results = index.query_callsites("print");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].callee, "print");
    }

    #[test]
    fn test_query_callsites_pattern() {
        let index = build_test_index();
        let results = index.query_callsites("*"); // all calls
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_query_imports_exact() {
        let index = build_test_index();
        let results = index.query_imports("os.path");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_query_imports_pattern() {
        let index = build_test_index();
        let results = index.query_imports("*django*");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].module_path, "django.db.models");
    }

    #[test]
    fn test_query_strings_min_length() {
        let index = build_test_index();
        let results = index.query_strings(Some(5), None, None);
        assert_eq!(results.len(), 2); // "Hello, World!" and "config.yaml"
    }

    #[test]
    fn test_query_strings_contains() {
        let index = build_test_index();
        let results = index.query_strings(None, Some("yaml"), None);
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("yaml"));
    }

    #[test]
    fn test_query_strings_file_pattern() {
        let index = build_test_index();
        let results = index.query_strings(None, None, Some("*config*"));
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_query_files() {
        let index = build_test_index();
        let results = index.query_files("src/*.py");
        assert_eq!(results.len(), 6);
    }

    #[test]
    fn test_query_files_specific() {
        let index = build_test_index();
        let results = index.query_files("*models*");
        assert_eq!(results.len(), 1);
        assert!(results[0].contains("models"));
    }

    #[test]
    fn test_query_decorated() {
        let index = build_test_index();
        let results = index.query_decorated("staticmethod");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "my_method");
    }

    #[test]
    fn test_query_decorated_pattern() {
        let index = build_test_index();
        let results = index.query_decorated("static*");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_read_file_section() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "line 1").unwrap();
        writeln!(file, "line 2").unwrap();
        writeln!(file, "line 3").unwrap();
        writeln!(file, "line 4").unwrap();
        writeln!(file, "line 5").unwrap();

        let path = file.path().to_string_lossy().to_string();

        // Read lines 2-4 (1-indexed)
        let result = read_file_section(&path, 2, 4);
        assert_eq!(result, "line 2\nline 3\nline 4");
    }

    #[test]
    fn test_read_file_section_nonexistent() {
        let result = read_file_section("/nonexistent/path.txt", 1, 10);
        assert_eq!(result, "");
    }

    #[test]
    fn test_read_file_section_out_of_range() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "line 1").unwrap();
        writeln!(file, "line 2").unwrap();

        let path = file.path().to_string_lossy().to_string();

        // Request lines beyond file length
        let result = read_file_section(&path, 100, 200);
        assert_eq!(result, "");
    }
}
