//! Symbol index data structures and inverted indices.
//!
//! Provides primary storage for extracted AST entities (symbols, imports,
//! call sites, etc.) along with inverted indices for fast lookup.
//!
//! Design reference: Sections 2.1, 2.2

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::languages::Language;

/// Source location within a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    /// Relative path from repo root.
    pub file: String,
    /// 1-indexed start line.
    pub line: u32,
    /// 1-indexed end line.
    pub end_line: u32,
    /// 0-indexed column.
    pub column: u32,
}

impl Location {
    pub fn new(file: impl Into<String>, line: u32, end_line: u32, column: u32) -> Self {
        Self {
            file: file.into(),
            line,
            end_line,
            column,
        }
    }
}

/// Classification of extracted symbols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymbolKind {
    Class,
    Function,
    Method,
    Type,
    Constant,
}

impl SymbolKind {
    /// Parse from string representation.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "class" => Some(Self::Class),
            "function" => Some(Self::Function),
            "method" => Some(Self::Method),
            "type" => Some(Self::Type),
            "constant" => Some(Self::Constant),
            _ => None,
        }
    }

    /// String representation for serialization.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Class => "class",
            Self::Function => "function",
            Self::Method => "method",
            Self::Type => "type",
            Self::Constant => "constant",
        }
    }
}

/// A named code entity extracted from source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    /// Symbol name (e.g., class name, function name).
    pub name: String,
    /// Classification of the symbol.
    pub kind: SymbolKind,
    /// Source location.
    pub location: Location,
    /// Programming language.
    pub language: Language,
    /// Containing class/module name, if any.
    pub parent: Option<String>,
    /// Decorator/attribute names applied to this symbol.
    pub decorators: Vec<String>,
    /// Full source text of the symbol (function body, class definition, etc.).
    pub source_text: String,
}

impl Symbol {
    pub fn new(
        name: impl Into<String>,
        kind: SymbolKind,
        location: Location,
        language: Language,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            location,
            language,
            parent: None,
            decorators: Vec::new(),
            source_text: String::new(),
        }
    }

    pub fn with_parent(mut self, parent: impl Into<String>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    pub fn with_decorators(mut self, decorators: Vec<String>) -> Self {
        self.decorators = decorators;
        self
    }

    pub fn with_source_text(mut self, source_text: impl Into<String>) -> Self {
        self.source_text = source_text.into();
        self
    }
}

/// Inheritance relationship between classes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inheritance {
    /// Name of the derived class.
    pub child_class: String,
    /// Name of the base class.
    pub parent_class: String,
    /// Location of the inheritance declaration.
    pub location: Location,
}

impl Inheritance {
    pub fn new(
        child_class: impl Into<String>,
        parent_class: impl Into<String>,
        location: Location,
    ) -> Self {
        Self {
            child_class: child_class.into(),
            parent_class: parent_class.into(),
            location,
        }
    }
}

/// An import/use statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Import {
    /// Module or package path being imported.
    pub module_path: String,
    /// Specific names imported (empty if importing whole module).
    pub imported_names: Vec<String>,
    /// Source location.
    pub location: Location,
}

impl Import {
    pub fn new(module_path: impl Into<String>, location: Location) -> Self {
        Self {
            module_path: module_path.into(),
            imported_names: Vec::new(),
            location,
        }
    }

    pub fn with_names(mut self, names: Vec<String>) -> Self {
        self.imported_names = names;
        self
    }
}

/// A function/method call site.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallSite {
    /// Name of the called function/method.
    pub callee: String,
    /// Source location of the call.
    pub location: Location,
    /// Enclosing function name, if any.
    pub in_function: Option<String>,
}

impl CallSite {
    pub fn new(callee: impl Into<String>, location: Location) -> Self {
        Self {
            callee: callee.into(),
            location,
            in_function: None,
        }
    }

    pub fn with_enclosing_function(mut self, func: impl Into<String>) -> Self {
        self.in_function = Some(func.into());
        self
    }
}

/// A string literal found in source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringLiteral {
    /// Content of the string (without quotes).
    pub content: String,
    /// Source location.
    pub location: Location,
}

impl StringLiteral {
    pub fn new(content: impl Into<String>, location: Location) -> Self {
        Self {
            content: content.into(),
            location,
        }
    }
}

/// Primary index storing all extracted entities with inverted indices for fast lookup.
#[derive(Debug, Default)]
pub struct SymbolIndex {
    // Primary storage
    symbols: Vec<Symbol>,
    inheritances: Vec<Inheritance>,
    imports: Vec<Import>,
    call_sites: Vec<CallSite>,
    strings: Vec<StringLiteral>,

    // Inverted indices for symbols
    symbols_by_name: HashMap<String, Vec<usize>>,
    symbols_by_kind: HashMap<SymbolKind, Vec<usize>>,
    symbols_by_file: BTreeMap<String, Vec<usize>>,

    // Inheritance graph
    /// Maps parent class name to indices of Inheritance records where it's the parent.
    subclasses: HashMap<String, Vec<usize>>,
    /// Maps child class name to indices of Inheritance records where it's the child.
    superclasses: HashMap<String, Vec<usize>>,

    // State
    /// Files read cleanly. Narrower than it looks: a file the reader could not
    /// fully follow belongs in `files_degraded` instead, never here and never
    /// in both.
    files_parsed: HashSet<String>,
    /// Files read with parts the reader could not follow.
    ///
    /// These still contributed symbols and calls, which is exactly why they
    /// need naming. A file that produced nothing stands out; one that produced
    /// most of its content and silently dropped the rest does not.
    files_degraded: HashSet<String>,
}

impl SymbolIndex {
    /// Create a new empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a symbol to the index.
    ///
    /// Returns the index of the newly added symbol.
    pub fn add_symbol(&mut self, symbol: Symbol) -> usize {
        let idx = self.symbols.len();

        // Update inverted indices
        self.symbols_by_name
            .entry(symbol.name.clone())
            .or_default()
            .push(idx);

        self.symbols_by_kind
            .entry(symbol.kind)
            .or_default()
            .push(idx);

        self.symbols_by_file
            .entry(symbol.location.file.clone())
            .or_default()
            .push(idx);

        self.symbols.push(symbol);
        idx
    }

    /// Add an inheritance relationship.
    pub fn add_inheritance(&mut self, inheritance: Inheritance) {
        let idx = self.inheritances.len();

        self.subclasses
            .entry(inheritance.parent_class.clone())
            .or_default()
            .push(idx);

        self.superclasses
            .entry(inheritance.child_class.clone())
            .or_default()
            .push(idx);

        self.inheritances.push(inheritance);
    }

    /// Add an import statement.
    pub fn add_import(&mut self, import: Import) {
        self.imports.push(import);
    }

    /// Add a call site.
    pub fn add_call_site(&mut self, call_site: CallSite) {
        self.call_sites.push(call_site);
    }

    /// Add a string literal.
    pub fn add_string(&mut self, literal: StringLiteral) {
        self.strings.push(literal);
    }

    /// Mark a file as having been read cleanly.
    ///
    /// Removes it from the degraded set, so that re-reading a file whose
    /// condition has changed moves it rather than listing it twice.
    pub fn mark_file_parsed(&mut self, file: &str) {
        self.files_degraded.remove(file);
        self.files_parsed.insert(file.to_string());
    }

    /// Mark a file as read with parts the reader could not follow.
    ///
    /// Removes it from the clean set, for the same reason.
    pub fn mark_file_degraded(&mut self, file: &str) {
        self.files_parsed.remove(file);
        self.files_degraded.insert(file.to_string());
    }

    /// Check if a file has been read cleanly.
    pub fn is_file_parsed(&self, file: &str) -> bool {
        self.files_parsed.contains(file)
    }

    /// Check if a file was read with parts the reader could not follow.
    pub fn is_file_degraded(&self, file: &str) -> bool {
        self.files_degraded.contains(file)
    }

    // Accessors for query implementations (Task 11)

    /// Get all symbols.
    pub fn symbols(&self) -> &[Symbol] {
        &self.symbols
    }

    /// Get symbol by index.
    pub fn get_symbol(&self, idx: usize) -> Option<&Symbol> {
        self.symbols.get(idx)
    }

    /// Get symbol indices by name.
    pub fn symbols_by_name(&self, name: &str) -> &[usize] {
        self.symbols_by_name
            .get(name)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Get symbol indices by kind.
    pub fn symbols_by_kind(&self, kind: SymbolKind) -> &[usize] {
        self.symbols_by_kind
            .get(&kind)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Get symbol indices by file.
    pub fn symbols_in_file(&self, file: &str) -> &[usize] {
        self.symbols_by_file
            .get(file)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Get all inheritances.
    pub fn inheritances(&self) -> &[Inheritance] {
        &self.inheritances
    }

    /// Get inheritance by index.
    pub fn get_inheritance(&self, idx: usize) -> Option<&Inheritance> {
        self.inheritances.get(idx)
    }

    /// Get inheritance indices where the given class is a parent (i.e., get subclass records).
    pub fn subclass_indices(&self, parent_class: &str) -> &[usize] {
        self.subclasses
            .get(parent_class)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Get inheritance indices where the given class is a child (i.e., get superclass records).
    pub fn superclass_indices(&self, child_class: &str) -> &[usize] {
        self.superclasses
            .get(child_class)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Get all imports.
    pub fn imports(&self) -> &[Import] {
        &self.imports
    }

    /// Get all call sites.
    pub fn call_sites(&self) -> &[CallSite] {
        &self.call_sites
    }

    /// Get all string literals.
    pub fn strings(&self) -> &[StringLiteral] {
        &self.strings
    }

    /// Get count of parsed files.
    pub fn files_parsed_count(&self) -> usize {
        self.files_parsed.len()
    }

    /// Get set of files read cleanly.
    pub fn files_parsed(&self) -> &HashSet<String> {
        &self.files_parsed
    }

    /// Get count of files read with parts the reader could not follow.
    pub fn files_degraded_count(&self) -> usize {
        self.files_degraded.len()
    }

    /// Get set of files read with parts the reader could not follow.
    ///
    /// Anything a caller reports from this index is incomplete by whatever
    /// these files contained, so a summary that omits them overstates itself.
    pub fn files_degraded(&self) -> &HashSet<String> {
        &self.files_degraded
    }

    /// Get statistics about the index.
    pub fn stats(&self) -> IndexStats {
        IndexStats {
            symbols: self.symbols.len(),
            inheritances: self.inheritances.len(),
            imports: self.imports.len(),
            call_sites: self.call_sites.len(),
            strings: self.strings.len(),
            files_parsed: self.files_parsed.len(),
            files_degraded: self.files_degraded.len(),
        }
    }
}

/// Statistics about an index.
#[derive(Debug, Clone, Copy)]
pub struct IndexStats {
    pub symbols: usize,
    pub inheritances: usize,
    pub imports: usize,
    pub call_sites: usize,
    pub strings: usize,
    /// Files read cleanly.
    pub files_parsed: usize,
    /// Files read with parts the reader could not follow. A run reporting a
    /// non-zero value here is reporting an incomplete result, whatever the
    /// other counts say.
    pub files_degraded: usize,
}

/// Serializable representation of index data (primary storage only).
/// Inverted indices are rebuilt on load.
#[derive(Serialize, Deserialize)]
struct IndexData {
    symbols: Vec<Symbol>,
    inheritances: Vec<Inheritance>,
    imports: Vec<Import>,
    call_sites: Vec<CallSite>,
    strings: Vec<StringLiteral>,
    files_parsed: HashSet<String>,
    #[serde(default)]
    files_degraded: HashSet<String>,
}

impl SymbolIndex {
    /// Save the index to a file using bincode serialization.
    pub fn save_to_file(&self, path: &Path) -> std::io::Result<()> {
        let data = IndexData {
            symbols: self.symbols.clone(),
            inheritances: self.inheritances.clone(),
            imports: self.imports.clone(),
            call_sites: self.call_sites.clone(),
            strings: self.strings.clone(),
            files_parsed: self.files_parsed.clone(),
            files_degraded: self.files_degraded.clone(),
        };

        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        bincode::serialize_into(writer, &data)
            .map_err(std::io::Error::other)
    }

    /// Load an index from a file, rebuilding inverted indices.
    pub fn load_from_file(path: &Path) -> std::io::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let data: IndexData = bincode::deserialize_from(reader)
            .map_err(std::io::Error::other)?;

        let mut index = Self {
            symbols: Vec::new(),
            inheritances: Vec::new(),
            imports: data.imports,
            call_sites: data.call_sites,
            strings: data.strings,
            symbols_by_name: HashMap::new(),
            symbols_by_kind: HashMap::new(),
            symbols_by_file: BTreeMap::new(),
            subclasses: HashMap::new(),
            superclasses: HashMap::new(),
            files_parsed: data.files_parsed,
            files_degraded: data.files_degraded,
        };

        // Rebuild symbol indices by re-adding each symbol
        for symbol in data.symbols {
            index.add_symbol(symbol);
        }

        // Rebuild inheritance indices
        for inheritance in data.inheritances {
            index.add_inheritance(inheritance);
        }

        Ok(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_location() -> Location {
        Location::new("src/main.rs", 10, 15, 4)
    }

    #[test]
    fn test_location_new() {
        let loc = Location::new("test.py", 1, 5, 0);
        assert_eq!(loc.file, "test.py");
        assert_eq!(loc.line, 1);
        assert_eq!(loc.end_line, 5);
        assert_eq!(loc.column, 0);
    }

    #[test]
    fn test_symbol_kind_parse() {
        assert_eq!(SymbolKind::parse("class"), Some(SymbolKind::Class));
        assert_eq!(SymbolKind::parse("CLASS"), Some(SymbolKind::Class));
        assert_eq!(SymbolKind::parse("function"), Some(SymbolKind::Function));
        assert_eq!(SymbolKind::parse("method"), Some(SymbolKind::Method));
        assert_eq!(SymbolKind::parse("type"), Some(SymbolKind::Type));
        assert_eq!(SymbolKind::parse("constant"), Some(SymbolKind::Constant));
        assert_eq!(SymbolKind::parse("unknown"), None);
    }

    #[test]
    fn test_symbol_kind_as_str() {
        assert_eq!(SymbolKind::Class.as_str(), "class");
        assert_eq!(SymbolKind::Function.as_str(), "function");
        assert_eq!(SymbolKind::Method.as_str(), "method");
        assert_eq!(SymbolKind::Type.as_str(), "type");
        assert_eq!(SymbolKind::Constant.as_str(), "constant");
    }

    #[test]
    fn test_symbol_builder() {
        let loc = sample_location();
        let symbol = Symbol::new("MyClass", SymbolKind::Class, loc, Language::Rust)
            .with_parent("module")
            .with_decorators(vec!["derive".to_string()]);

        assert_eq!(symbol.name, "MyClass");
        assert_eq!(symbol.kind, SymbolKind::Class);
        assert_eq!(symbol.parent, Some("module".to_string()));
        assert_eq!(symbol.decorators, vec!["derive"]);
    }

    #[test]
    fn test_inheritance_new() {
        let loc = sample_location();
        let inh = Inheritance::new("Child", "Parent", loc.clone());
        assert_eq!(inh.child_class, "Child");
        assert_eq!(inh.parent_class, "Parent");
        assert_eq!(inh.location.file, loc.file);
    }

    #[test]
    fn test_import_builder() {
        let loc = sample_location();
        let imp = Import::new("std::collections", loc).with_names(vec!["HashMap".to_string()]);
        assert_eq!(imp.module_path, "std::collections");
        assert_eq!(imp.imported_names, vec!["HashMap"]);
    }

    #[test]
    fn test_call_site_builder() {
        let loc = sample_location();
        let cs = CallSite::new("process", loc).with_enclosing_function("main");
        assert_eq!(cs.callee, "process");
        assert_eq!(cs.in_function, Some("main".to_string()));
    }

    #[test]
    fn test_string_literal_new() {
        let loc = sample_location();
        let sl = StringLiteral::new("hello world", loc.clone());
        assert_eq!(sl.content, "hello world");
        assert_eq!(sl.location.file, loc.file);
    }

    #[test]
    fn test_symbol_index_add_symbol() {
        let mut index = SymbolIndex::new();
        let loc = sample_location();
        let symbol = Symbol::new("MyFunc", SymbolKind::Function, loc, Language::Python);

        let idx = index.add_symbol(symbol);
        assert_eq!(idx, 0);

        // Check primary storage
        assert_eq!(index.symbols().len(), 1);
        assert_eq!(index.get_symbol(0).unwrap().name, "MyFunc");

        // Check inverted indices
        assert_eq!(index.symbols_by_name("MyFunc"), &[0]);
        assert_eq!(index.symbols_by_kind(SymbolKind::Function), &[0]);
        assert_eq!(index.symbols_in_file("src/main.rs"), &[0]);
    }

    #[test]
    fn test_symbol_index_multiple_symbols_same_name() {
        let mut index = SymbolIndex::new();

        let loc1 = Location::new("file1.py", 1, 5, 0);
        let loc2 = Location::new("file2.py", 10, 15, 0);

        index.add_symbol(Symbol::new("process", SymbolKind::Function, loc1, Language::Python));
        index.add_symbol(Symbol::new("process", SymbolKind::Function, loc2, Language::Python));

        assert_eq!(index.symbols_by_name("process"), &[0, 1]);
    }

    #[test]
    fn test_symbol_index_inheritance() {
        let mut index = SymbolIndex::new();
        let loc = sample_location();

        index.add_inheritance(Inheritance::new("Dog", "Animal", loc.clone()));
        index.add_inheritance(Inheritance::new("Cat", "Animal", loc));

        // Animal is parent of Dog and Cat
        assert_eq!(index.subclass_indices("Animal").len(), 2);

        // Dog has one parent
        assert_eq!(index.superclass_indices("Dog"), &[0]);

        // Cat has one parent
        assert_eq!(index.superclass_indices("Cat"), &[1]);
    }

    #[test]
    fn test_symbol_index_imports_and_calls() {
        let mut index = SymbolIndex::new();
        let loc = sample_location();

        index.add_import(Import::new("os.path", loc.clone()));
        index.add_call_site(CallSite::new("join", loc.clone()));
        index.add_string(StringLiteral::new("/tmp", loc));

        assert_eq!(index.imports().len(), 1);
        assert_eq!(index.call_sites().len(), 1);
        assert_eq!(index.strings().len(), 1);
    }

    #[test]
    fn test_symbol_index_file_tracking() {
        let mut index = SymbolIndex::new();

        assert!(!index.is_file_parsed("test.py"));
        assert_eq!(index.files_parsed_count(), 0);

        index.mark_file_parsed("test.py");

        assert!(index.is_file_parsed("test.py"));
        assert_eq!(index.files_parsed_count(), 1);

        // Marking again should be idempotent
        index.mark_file_parsed("test.py");
        assert_eq!(index.files_parsed_count(), 1);
    }

    #[test]
    fn test_symbol_index_stats() {
        let mut index = SymbolIndex::new();
        let loc = sample_location();

        index.add_symbol(Symbol::new("Foo", SymbolKind::Class, loc.clone(), Language::Python));
        index.add_inheritance(Inheritance::new("Bar", "Foo", loc.clone()));
        index.add_import(Import::new("sys", loc.clone()));
        index.add_call_site(CallSite::new("exit", loc.clone()));
        index.add_string(StringLiteral::new("test", loc));
        index.mark_file_parsed("test.py");

        let stats = index.stats();
        assert_eq!(stats.symbols, 1);
        assert_eq!(stats.inheritances, 1);
        assert_eq!(stats.imports, 1);
        assert_eq!(stats.call_sites, 1);
        assert_eq!(stats.strings, 1);
        assert_eq!(stats.files_parsed, 1);
    }

    #[test]
    fn test_empty_index_queries() {
        let index = SymbolIndex::new();

        // All queries on empty index should return empty results
        assert!(index.symbols_by_name("foo").is_empty());
        assert!(index.symbols_by_kind(SymbolKind::Class).is_empty());
        assert!(index.symbols_in_file("test.py").is_empty());
        assert!(index.subclass_indices("Parent").is_empty());
        assert!(index.superclass_indices("Child").is_empty());
    }
}
