//! Tree-sitter parsing and AST entity extraction.
//!
//! Parses source files using tree-sitter and populates the SymbolIndex
//! with extracted symbols, imports, call sites, inheritance, and strings.
//!
//! Design reference: Sections 2.1, 2.2

use std::fs;
use std::path::Path;
use std::time::Instant;

use tree_sitter::Parser as TsParser;

use crate::index::{
    CallSite, Import, Inheritance, Location, StringLiteral, Symbol, SymbolIndex, SymbolKind,
};
use crate::languages::Language;

/// Statistics from parsing a single file.
#[derive(Debug, Clone, Default)]
pub struct ParseStats {
    pub symbols_found: usize,
    pub imports_found: usize,
    pub call_sites_found: usize,
    pub strings_found: usize,
    pub inheritances_found: usize,
    pub parse_time_ms: u64,
    /// Number of functions/methods parsed.
    pub function_count: usize,
    /// Total cyclomatic complexity across all functions.
    pub total_cc: usize,
    /// Maximum cyclomatic complexity of any single function.
    pub max_function_cc: usize,
    /// Number of ERROR or MISSING nodes in the parsed tree.
    ///
    /// Zero means the reader made complete sense of the file. Anything above
    /// zero means it did not, and whatever sat in those regions contributed
    /// nothing. Tree-sitter recovers rather than failing, so without this a
    /// file the reader could not follow is indistinguishable from one it read
    /// perfectly.
    ///
    /// A count rather than a flag, deliberately. `has_error()` on the root is
    /// constant time and yields a boolean, which cannot separate a file with
    /// one stray token from a file the reader abandoned. That separation is
    /// the point, so the cost of one predicate per node is accepted.
    pub error_nodes: usize,
}

/// Error during parsing.
#[derive(Debug)]
pub enum ParseError {
    /// File could not be read.
    IoError(std::io::Error),
    /// Tree-sitter failed to parse the file.
    ParseFailed,
    /// Could not set tree-sitter language.
    LanguageError,
}

impl From<std::io::Error> for ParseError {
    fn from(e: std::io::Error) -> Self {
        ParseError::IoError(e)
    }
}

/// Parse a file and populate the index.
///
/// Args:
///   abs_path: Absolute path for reading the file from disk
///   rel_path: Relative path to store in the index (relative to repo root)
///   language: The programming language
///   index: Symbol index to populate
///
/// Returns statistics about what was extracted.
pub fn parse_file(
    abs_path: &Path,
    rel_path: &Path,
    language: Language,
    index: &mut SymbolIndex,
) -> Result<ParseStats, ParseError> {
    let start = Instant::now();

    let content = fs::read(abs_path)?;
    let file_str = rel_path.to_string_lossy().to_string();

    let mut parser = TsParser::new();
    parser
        .set_language(language.tree_sitter_language())
        .map_err(|_| ParseError::LanguageError)?;

    let tree = parser.parse(&content, None).ok_or(ParseError::ParseFailed)?;

    let mut stats = ParseStats::default();
    let mut context = VisitorContext {
        source: &content,
        language,
        file: &file_str,
        parent_class: None,
        enclosing_function: None,
        pending_decorators: Vec::new(),
        current_function_cc: 0,
    };

    visit_node(tree.root_node(), &mut context, index, &mut stats);

    // One or the other, never both and never neither. A file that produced
    // results while the reader was lost is the case the split exists for: its
    // counts look ordinary and its contribution is not.
    if stats.error_nodes > 0 {
        index.mark_file_degraded(&file_str);
    } else {
        index.mark_file_parsed(&file_str);
    }

    stats.parse_time_ms = start.elapsed().as_millis() as u64;
    Ok(stats)
}

/// Context passed through the AST traversal.
struct VisitorContext<'a> {
    source: &'a [u8],
    language: Language,
    file: &'a str,
    parent_class: Option<String>,
    enclosing_function: Option<String>,
    pending_decorators: Vec<String>,
    /// Cyclomatic complexity of current function (1 = base, +1 per decision point).
    current_function_cc: usize,
}

/// Extract text from a node.
fn get_node_text<'a>(node: &tree_sitter::Node, source: &'a [u8]) -> &'a str {
    std::str::from_utf8(&source[node.byte_range()]).unwrap_or("")
}

/// Create a Location from a tree-sitter node.
fn node_location(node: &tree_sitter::Node, file: &str) -> Location {
    Location::new(
        file,
        node.start_position().row as u32 + 1,
        node.end_position().row as u32 + 1,
        node.start_position().column as u32,
    )
}

/// Check if a node is a boolean operator (&&, ||, and, or).
fn is_boolean_operator(node: &tree_sitter::Node, ctx: &VisitorContext) -> bool {
    let operators = ctx.language.boolean_operators();

    // Try to find operator field first (common in JS/TS/Go/Rust/Java)
    if let Some(op_node) = node.child_by_field_name("operator") {
        let op_text = get_node_text(&op_node, ctx.source);
        return operators.contains(&op_text);
    }

    // For Python boolean_operator, the operator is a direct child
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            let text = get_node_text(&child, ctx.source);
            if operators.contains(&text) {
                return true;
            }
        }
    }

    false
}

/// Recursive AST visitor.
fn visit_node(
    node: tree_sitter::Node,
    ctx: &mut VisitorContext,
    index: &mut SymbolIndex,
    stats: &mut ParseStats,
) {
    let kind = node.kind();

    // Counted on the walk that is happening anyway, rather than in a second
    // traversal. MISSING counts alongside ERROR: a node the parser inserted to
    // repair the tree marks the same thing, a region it could not follow.
    if node.is_error() || node.is_missing() {
        stats.error_nodes += 1;
    }

    // Check if this is a decorator
    if ctx.language.decorator_node_types().contains(&kind) {
        if let Some(name) = extract_decorator_name(&node, ctx.source, ctx.language) {
            ctx.pending_decorators.push(name);
        }
    }

    // Check if this is a class definition
    if ctx.language.class_node_types().contains(&kind) {
        let name = extract_class_name(&node, ctx.source, ctx.language);
        if let Some(name) = name {
            let location = node_location(&node, ctx.file);
            let source_text = get_node_text(&node, ctx.source).to_string();

            // Create symbol
            let mut symbol = Symbol::new(name.clone(), SymbolKind::Class, location, ctx.language)
                .with_source_text(source_text);
            if !ctx.pending_decorators.is_empty() {
                symbol = symbol.with_decorators(std::mem::take(&mut ctx.pending_decorators));
            }
            index.add_symbol(symbol);
            stats.symbols_found += 1;

            // Extract inheritance
            extract_inheritance(&node, ctx, &name, index, stats);

            // Visit children with this class as parent
            let old_parent = ctx.parent_class.take();
            ctx.parent_class = Some(name);

            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    visit_node(child, ctx, index, stats);
                }
            }

            ctx.parent_class = old_parent;
            return;
        }
    }

    // Check if this is a function definition
    if ctx.language.function_node_types().contains(&kind) {
        let name = extract_function_name(&node, ctx.source);
        if let Some(name) = name {
            let location = node_location(&node, ctx.file);
            let source_text = get_node_text(&node, ctx.source).to_string();

            // Determine if it's a method (has parent class)
            let symbol_kind = if ctx.parent_class.is_some() {
                SymbolKind::Method
            } else {
                SymbolKind::Function
            };

            let mut symbol = Symbol::new(name.clone(), symbol_kind, location, ctx.language)
                .with_source_text(source_text);
            if let Some(ref parent) = ctx.parent_class {
                symbol = symbol.with_parent(parent.clone());
            }
            if !ctx.pending_decorators.is_empty() {
                symbol = symbol.with_decorators(std::mem::take(&mut ctx.pending_decorators));
            }
            index.add_symbol(symbol);
            stats.symbols_found += 1;
            stats.function_count += 1;

            // Visit children with this function as enclosing
            let old_enclosing = ctx.enclosing_function.take();
            let old_cc = ctx.current_function_cc;
            ctx.enclosing_function = Some(name);
            ctx.current_function_cc = 1; // Base complexity

            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    visit_node(child, ctx, index, stats);
                }
            }

            // Record function's complexity
            let function_cc = ctx.current_function_cc;
            stats.total_cc += function_cc;
            if function_cc > stats.max_function_cc {
                stats.max_function_cc = function_cc;
            }

            ctx.enclosing_function = old_enclosing;
            ctx.current_function_cc = old_cc;
            return;
        }
    }

    // Check if this is an import statement
    if ctx.language.import_node_types().contains(&kind) {
        if let Some(import) = extract_import(&node, ctx) {
            index.add_import(import);
            stats.imports_found += 1;
        }
    }

    // Check if this is a call site
    if ctx.language.call_node_types().contains(&kind) {
        if let Some(call_site) = extract_call_site(&node, ctx) {
            index.add_call_site(call_site);
            stats.call_sites_found += 1;
        }
    }

    // Check if this is a string literal
    if ctx.language.string_node_types().contains(&kind) {
        if let Some(string_lit) = extract_string_literal(&node, ctx) {
            index.add_string(string_lit);
            stats.strings_found += 1;
        }
    }

    // Count decision points for cyclomatic complexity (only when inside a function)
    if ctx.enclosing_function.is_some() {
        // Check for decision point nodes (if, for, while, etc.)
        if ctx.language.decision_point_types().contains(&kind) {
            ctx.current_function_cc += 1;
        }

        // Check for boolean operators (&& or ||)
        if ctx.language.boolean_operator_node_types().contains(&kind)
            && is_boolean_operator(&node, ctx)
        {
            ctx.current_function_cc += 1;
        }
    }

    // Visit children
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            visit_node(child, ctx, index, stats);
        }
    }
}

/// Extract class name from a class definition node.
fn extract_class_name(
    node: &tree_sitter::Node,
    source: &[u8],
    language: Language,
) -> Option<String> {
    // Try common field names first
    for field in &["name", "declarator"] {
        if let Some(child) = node.child_by_field_name(field) {
            let text = get_node_text(&child, source);
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }

    // For Go type declarations, look for type_spec child
    if language == Language::Go {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.kind() == "type_spec" {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        return Some(get_node_text(&name_node, source).to_string());
                    }
                }
            }
        }
    }

    // Fallback: find first identifier child
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "identifier" || child.kind() == "type_identifier" {
                return Some(get_node_text(&child, source).to_string());
            }
        }
    }

    None
}

/// Extract function name from a function definition node.
fn extract_function_name(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    // Try common field names
    for field in &["name", "declarator"] {
        if let Some(child) = node.child_by_field_name(field) {
            // Handle function_declarator (C-style)
            if child.kind() == "function_declarator" {
                if let Some(name) = child.child_by_field_name("declarator") {
                    return Some(get_node_text(&name, source).to_string());
                }
            }
            let text = get_node_text(&child, source);
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }

    // Fallback: find identifier child
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "identifier" {
                return Some(get_node_text(&child, source).to_string());
            }
        }
    }

    // For arrow functions and anonymous functions, return a placeholder
    if node.kind() == "arrow_function" || node.kind() == "function_expression" {
        return Some("<anonymous>".to_string());
    }

    None
}

/// Extract decorator/attribute name.
fn extract_decorator_name(
    node: &tree_sitter::Node,
    source: &[u8],
    language: Language,
) -> Option<String> {
    match language {
        Language::Python => {
            // Python decorator: @name or @name.attr
            if let Some(expr) = node.child(1) {
                // Skip the @ symbol
                return Some(get_node_text(&expr, source).to_string());
            }
        }
        Language::Rust => {
            // Rust attribute: #[name] or #[name(...)]
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if child.kind() == "attribute" || child.kind() == "meta_item" {
                        if let Some(path) = child.child_by_field_name("path") {
                            return Some(get_node_text(&path, source).to_string());
                        }
                        // Fallback: first identifier
                        for j in 0..child.child_count() {
                            if let Some(gc) = child.child(j) {
                                if gc.kind() == "identifier" {
                                    return Some(get_node_text(&gc, source).to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        Language::Java => {
            // Java annotation: @Name
            if let Some(name_node) = node.child_by_field_name("name") {
                return Some(get_node_text(&name_node, source).to_string());
            }
            // Fallback
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if child.kind() == "identifier" {
                        return Some(get_node_text(&child, source).to_string());
                    }
                }
            }
        }
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            // JS/TS decorator: @name
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if child.kind() == "identifier" || child.kind() == "call_expression" {
                        return Some(get_node_text(&child, source).to_string());
                    }
                }
            }
        }
        Language::Go => {
            // Go doesn't have decorators
        }
    }
    None
}

/// Extract inheritance relationships from a class node.
fn extract_inheritance(
    node: &tree_sitter::Node,
    ctx: &VisitorContext,
    child_class: &str,
    index: &mut SymbolIndex,
    stats: &mut ParseStats,
) {
    let location = node_location(node, ctx.file);

    match ctx.language {
        Language::Python => {
            // Python: class Foo(Bar, Baz)
            if let Some(bases) = node.child_by_field_name("superclasses") {
                for i in 0..bases.child_count() {
                    if let Some(base) = bases.child(i) {
                        let kind = base.kind();
                        // Only process actual type references, not punctuation
                        if kind == "identifier"
                            || kind == "attribute"
                            || kind == "subscript"
                            || kind == "call"
                        {
                            let text = get_node_text(&base, ctx.source);
                            if !text.is_empty() {
                                index.add_inheritance(Inheritance::new(
                                    child_class,
                                    text,
                                    location.clone(),
                                ));
                                stats.inheritances_found += 1;
                            }
                        } else if kind == "argument_list" {
                            // Handle argument_list children (older tree-sitter versions)
                            for j in 0..base.child_count() {
                                if let Some(arg) = base.child(j) {
                                    let arg_kind = arg.kind();
                                    if arg_kind == "identifier"
                                        || arg_kind == "attribute"
                                        || arg_kind == "subscript"
                                        || arg_kind == "call"
                                    {
                                        let text = get_node_text(&arg, ctx.source);
                                        if !text.is_empty() {
                                            index.add_inheritance(Inheritance::new(
                                                child_class,
                                                text,
                                                location.clone(),
                                            ));
                                            stats.inheritances_found += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            // JS/TS: class Foo extends Bar
            if let Some(heritage) = node.child_by_field_name("heritage") {
                // heritage is a class_heritage node
                for i in 0..heritage.child_count() {
                    if let Some(child) = heritage.child(i) {
                        if child.kind() == "extends_clause" {
                            // Get the class being extended
                            for j in 0..child.child_count() {
                                if let Some(gc) = child.child(j) {
                                    if gc.kind() == "identifier" || gc.kind() == "member_expression"
                                    {
                                        let text = get_node_text(&gc, ctx.source);
                                        if !text.is_empty() {
                                            index.add_inheritance(Inheritance::new(
                                                child_class,
                                                text,
                                                location.clone(),
                                            ));
                                            stats.inheritances_found += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Language::Java => {
            // Java: class Foo extends Bar implements Baz
            if let Some(superclass) = node.child_by_field_name("superclass") {
                let text = get_node_text(&superclass, ctx.source);
                if !text.is_empty() {
                    index.add_inheritance(Inheritance::new(child_class, text, location.clone()));
                    stats.inheritances_found += 1;
                }
            }
            if let Some(interfaces) = node.child_by_field_name("interfaces") {
                for i in 0..interfaces.child_count() {
                    if let Some(iface) = interfaces.child(i) {
                        if iface.kind() == "type_identifier" || iface.kind() == "generic_type" {
                            let text = get_node_text(&iface, ctx.source);
                            if !text.is_empty() {
                                index.add_inheritance(Inheritance::new(
                                    child_class,
                                    text,
                                    location.clone(),
                                ));
                                stats.inheritances_found += 1;
                            }
                        }
                    }
                }
            }
        }
        Language::Go | Language::Rust => {
            // Go uses embedding, Rust uses traits - handled differently or skipped for MVP
        }
    }
}

/// Extract import information.
fn extract_import(node: &tree_sitter::Node, ctx: &VisitorContext) -> Option<Import> {
    let location = node_location(node, ctx.file);

    match ctx.language {
        Language::Python => {
            // import foo or from foo import bar
            let kind = node.kind();
            if kind == "import_statement" {
                // import foo, bar
                let mut module_path = String::new();
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        if child.kind() == "dotted_name" {
                            module_path = get_node_text(&child, ctx.source).to_string();
                            break;
                        }
                    }
                }
                if !module_path.is_empty() {
                    return Some(Import::new(module_path, location));
                }
            } else if kind == "import_from_statement" {
                // from foo import bar, baz
                let mut module_path = String::new();
                let mut names = Vec::new();

                if let Some(module) = node.child_by_field_name("module_name") {
                    module_path = get_node_text(&module, ctx.source).to_string();
                }
                // Fallback: look for dotted_name or relative_import
                if module_path.is_empty() {
                    for i in 0..node.child_count() {
                        if let Some(child) = node.child(i) {
                            if child.kind() == "dotted_name" || child.kind() == "relative_import" {
                                module_path = get_node_text(&child, ctx.source).to_string();
                                break;
                            }
                        }
                    }
                }

                // Extract imported names
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        if child.kind() == "import_prefix" {
                            // Skip "from" keyword
                            continue;
                        }
                        if child.kind() == "identifier" || child.kind() == "aliased_import" {
                            let text = get_node_text(&child, ctx.source);
                            if text != "import" && text != "from" {
                                names.push(text.to_string());
                            }
                        }
                    }
                }

                if !module_path.is_empty() {
                    return Some(Import::new(module_path, location).with_names(names));
                }
            }
        }
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            // import foo from 'bar' or import { a, b } from 'bar'
            if let Some(source_node) = node.child_by_field_name("source") {
                let module_path = get_node_text(&source_node, ctx.source)
                    .trim_matches(|c| c == '"' || c == '\'')
                    .to_string();

                let mut names = Vec::new();
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        if child.kind() == "import_clause" || child.kind() == "named_imports" {
                            extract_import_names(&child, ctx.source, &mut names);
                        }
                    }
                }

                if !module_path.is_empty() {
                    return Some(Import::new(module_path, location).with_names(names));
                }
            }
        }
        Language::Go => {
            // import "path" or import ( "path1" "path2" )
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if child.kind() == "import_spec" || child.kind() == "import_spec_list" {
                        // Handle single or multiple imports
                        let text = get_node_text(&child, ctx.source);
                        let module_path = text
                            .trim_matches(|c| c == '"' || c == '(' || c == ')' || c == '\n')
                            .to_string();
                        if !module_path.is_empty() {
                            return Some(Import::new(module_path, location));
                        }
                    } else if child.kind() == "interpreted_string_literal" {
                        let module_path = get_node_text(&child, ctx.source)
                            .trim_matches('"')
                            .to_string();
                        return Some(Import::new(module_path, location));
                    }
                }
            }
        }
        Language::Rust => {
            // use foo::bar;
            if let Some(path) = node.child_by_field_name("argument") {
                let module_path = get_node_text(&path, ctx.source).to_string();
                return Some(Import::new(module_path, location));
            }
            // Fallback
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if child.kind() == "use_tree" || child.kind() == "scoped_identifier" {
                        let module_path = get_node_text(&child, ctx.source).to_string();
                        return Some(Import::new(module_path, location));
                    }
                }
            }
        }
        Language::Java => {
            // import foo.bar.Baz;
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if child.kind() == "scoped_identifier" {
                        let module_path = get_node_text(&child, ctx.source).to_string();
                        return Some(Import::new(module_path, location));
                    }
                }
            }
        }
    }

    None
}

/// Helper to extract import names from JS/TS import clauses.
fn extract_import_names(node: &tree_sitter::Node, source: &[u8], names: &mut Vec<String>) {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            match child.kind() {
                "identifier" => {
                    names.push(get_node_text(&child, source).to_string());
                }
                "import_specifier" => {
                    if let Some(name) = child.child_by_field_name("name") {
                        names.push(get_node_text(&name, source).to_string());
                    }
                }
                "named_imports" => {
                    extract_import_names(&child, source, names);
                }
                _ => {}
            }
        }
    }
}

/// Extract call site information.
fn extract_call_site(node: &tree_sitter::Node, ctx: &VisitorContext) -> Option<CallSite> {
    let location = node_location(node, ctx.file);

    // Get the callee (function being called)
    let callee = if let Some(func) = node.child_by_field_name("function") {
        get_node_text(&func, ctx.source).to_string()
    } else if let Some(callee_node) = node.child(0) {
        get_node_text(&callee_node, ctx.source).to_string()
    } else {
        return None;
    };

    if callee.is_empty() {
        return None;
    }

    let mut call_site = CallSite::new(callee, location);
    if let Some(ref func_name) = ctx.enclosing_function {
        call_site = call_site.with_enclosing_function(func_name.clone());
    }

    Some(call_site)
}

/// Extract string literal.
fn extract_string_literal(node: &tree_sitter::Node, ctx: &VisitorContext) -> Option<StringLiteral> {
    let raw_content = get_node_text(node, ctx.source);

    // Strip quotes
    let content = raw_content
        .trim_start_matches(['"', '\'', '`', 'r', 'b'])
        .trim_start_matches('#')
        .trim_start_matches('"')
        .trim_end_matches(['"', '\'', '`'])
        .trim_end_matches('#');

    // Skip empty strings and very short strings
    if content.len() < 2 {
        return None;
    }

    let location = node_location(node, ctx.file);
    Some(StringLiteral::new(content, location))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn parse_code(code: &str, language: Language) -> (SymbolIndex, ParseStats) {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(code.as_bytes()).unwrap();

        let mut index = SymbolIndex::new();
        // In tests, abs_path and rel_path are the same (temp file)
        let path = file.path();
        let stats = parse_file(path, path, language, &mut index).unwrap();

        (index, stats)
    }

    #[test]
    fn test_parse_python_class() {
        let code = r#"
class MyClass:
    def my_method(self):
        pass
"#;
        let (index, stats) = parse_code(code, Language::Python);

        assert_eq!(stats.symbols_found, 2); // class + method
        assert_eq!(index.symbols().len(), 2);

        let class_sym = &index.symbols()[0];
        assert_eq!(class_sym.name, "MyClass");
        assert_eq!(class_sym.kind, SymbolKind::Class);

        let method_sym = &index.symbols()[1];
        assert_eq!(method_sym.name, "my_method");
        assert_eq!(method_sym.kind, SymbolKind::Method);
        assert_eq!(method_sym.parent, Some("MyClass".to_string()));
    }

    #[test]
    fn test_parse_python_function() {
        let code = r#"
def standalone_func():
    pass
"#;
        let (index, stats) = parse_code(code, Language::Python);

        assert_eq!(stats.symbols_found, 1);
        let func = &index.symbols()[0];
        assert_eq!(func.name, "standalone_func");
        assert_eq!(func.kind, SymbolKind::Function);
        assert_eq!(func.parent, None);
    }

    #[test]
    fn test_parse_python_inheritance() {
        let code = r#"
class Child(Parent):
    pass
"#;
        let (index, stats) = parse_code(code, Language::Python);

        assert_eq!(stats.inheritances_found, 1);
        let inh = &index.inheritances()[0];
        assert_eq!(inh.child_class, "Child");
        assert_eq!(inh.parent_class, "Parent");
    }

    #[test]
    fn test_parse_python_import() {
        let code = r#"
import os
from collections import defaultdict
"#;
        let (index, stats) = parse_code(code, Language::Python);

        assert_eq!(stats.imports_found, 2);
        assert_eq!(index.imports().len(), 2);
    }

    #[test]
    fn test_parse_python_decorator() {
        let code = r#"
@staticmethod
def my_func():
    pass
"#;
        let (index, _stats) = parse_code(code, Language::Python);

        let func = &index.symbols()[0];
        assert_eq!(func.name, "my_func");
        assert!(func.decorators.contains(&"staticmethod".to_string()));
    }

    #[test]
    fn test_parse_python_call_site() {
        let code = r#"
def foo():
    bar()
    baz.qux()
"#;
        let (index, stats) = parse_code(code, Language::Python);

        assert!(stats.call_sites_found >= 2);
        let calls: Vec<_> = index.call_sites().iter().collect();
        assert!(calls.iter().any(|c| c.callee == "bar"));
    }

    #[test]
    fn test_parse_rust_function() {
        let code = r#"
fn main() {
    println!("hello");
}
"#;
        let (index, stats) = parse_code(code, Language::Rust);

        assert_eq!(stats.symbols_found, 1);
        let func = &index.symbols()[0];
        assert_eq!(func.name, "main");
        assert_eq!(func.kind, SymbolKind::Function);
    }

    #[test]
    fn test_parse_rust_struct() {
        let code = r#"
struct Point {
    x: i32,
    y: i32,
}
"#;
        let (index, stats) = parse_code(code, Language::Rust);

        assert_eq!(stats.symbols_found, 1);
        let sym = &index.symbols()[0];
        assert_eq!(sym.name, "Point");
        assert_eq!(sym.kind, SymbolKind::Class);
    }

    #[test]
    fn test_parse_javascript_class() {
        let code = r#"
class MyClass extends BaseClass {
    constructor() {}
    myMethod() {}
}
"#;
        let (index, stats) = parse_code(code, Language::JavaScript);

        assert!(stats.symbols_found >= 2); // class + methods
        let class_sym = index.symbols().iter().find(|s| s.name == "MyClass").unwrap();
        assert_eq!(class_sym.kind, SymbolKind::Class);
    }

    #[test]
    fn test_parse_go_function() {
        let code = r#"
package main

func main() {
    fmt.Println("hello")
}
"#;
        let (index, stats) = parse_code(code, Language::Go);

        assert_eq!(stats.symbols_found, 1);
        let func = &index.symbols()[0];
        assert_eq!(func.name, "main");
        assert_eq!(func.kind, SymbolKind::Function);
    }

    #[test]
    fn test_parse_java_class() {
        let code = r#"
public class MyClass {
    public void myMethod() {}
}
"#;
        let (index, stats) = parse_code(code, Language::Java);

        assert!(stats.symbols_found >= 1);
        let class_sym = index.symbols().iter().find(|s| s.name == "MyClass").unwrap();
        assert_eq!(class_sym.kind, SymbolKind::Class);
    }

    #[test]
    fn test_parse_strings() {
        let code = r#"
message = "Hello, World!"
"#;
        let (index, stats) = parse_code(code, Language::Python);

        assert!(stats.strings_found >= 1);
        let has_hello = index
            .strings()
            .iter()
            .any(|s| s.content.contains("Hello"));
        assert!(has_hello);
    }

    #[test]
    fn test_file_tracking() {
        let code = "def foo(): pass";
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(code.as_bytes()).unwrap();

        let mut index = SymbolIndex::new();
        let path = file.path();
        let path_str = path.to_string_lossy().to_string();

        assert!(!index.is_file_parsed(&path_str));

        parse_file(path, path, Language::Python, &mut index).unwrap();

        assert!(index.is_file_parsed(&path_str));
    }

    // === Edge Case Tests ===

    #[test]
    fn test_edge_empty_file() {
        let code = "";
        let (index, stats) = parse_code(code, Language::Python);

        assert_eq!(stats.symbols_found, 0);
        assert_eq!(index.symbols().len(), 0);
    }

    #[test]
    fn test_edge_whitespace_only() {
        let code = "   \n\n\t\t\n   ";
        let (index, stats) = parse_code(code, Language::Python);

        assert_eq!(stats.symbols_found, 0);
        assert_eq!(index.symbols().len(), 0);
    }

    #[test]
    fn test_edge_comments_only() {
        let code = r#"
# This is a comment
# Another comment

"""
A multiline
docstring comment
"""
"#;
        let (index, stats) = parse_code(code, Language::Python);

        // Should parse without error, may have string literals
        assert_eq!(stats.symbols_found, 0);
        assert_eq!(index.symbols().len(), 0);
    }

    #[test]
    fn test_edge_deeply_nested_classes() {
        let code = r#"
class Outer:
    class Middle:
        class Inner:
            def deep_method(self):
                pass
"#;
        let (index, stats) = parse_code(code, Language::Python);

        // Should handle nested classes
        assert!(stats.symbols_found >= 3); // At least 3 classes
    }

    #[test]
    fn test_edge_unicode_identifiers() {
        let code = r#"
def 函数名():
    pass

class Класс:
    pass
"#;
        let (index, stats) = parse_code(code, Language::Python);

        // Should parse unicode identifiers
        assert_eq!(stats.symbols_found, 2);
        assert!(index.symbols().iter().any(|s| s.name == "函数名"));
    }

    #[test]
    fn test_edge_very_long_function_name() {
        let name = "a".repeat(500);
        let code = format!("def {}(): pass", name);
        let (index, stats) = parse_code(&code, Language::Python);

        assert_eq!(stats.symbols_found, 1);
        assert_eq!(index.symbols()[0].name.len(), 500);
    }

    #[test]
    fn test_edge_many_parameters() {
        let params: Vec<String> = (0..100).map(|i| format!("arg{}", i)).collect();
        let code = format!("def many_params({}): pass", params.join(", "));
        let (index, stats) = parse_code(&code, Language::Python);

        assert_eq!(stats.symbols_found, 1);
        assert_eq!(index.symbols()[0].name, "many_params");
    }

    #[test]
    fn test_edge_multiple_inheritance() {
        let code = r#"
class Multi(Base1, Base2, Base3, mixin1.Mixin, Generic[T]):
    pass
"#;
        let (index, stats) = parse_code(code, Language::Python);

        // Should capture multiple parent classes
        assert!(stats.inheritances_found >= 3);
    }

    #[test]
    fn test_edge_decorator_chain() {
        let code = r#"
@decorator1
@decorator2.nested
@decorator3(with_args)
def decorated():
    pass
"#;
        let (index, _stats) = parse_code(code, Language::Python);

        let func = &index.symbols()[0];
        assert_eq!(func.name, "decorated");
        assert!(func.decorators.len() >= 1);
    }

    #[test]
    fn test_edge_lambda_in_class() {
        let code = r#"
class WithLambda:
    processor = lambda x: x * 2
    def method(self):
        return lambda: None
"#;
        let (index, stats) = parse_code(code, Language::Python);

        // Should handle lambdas without crashing
        assert!(stats.symbols_found >= 1);
    }

    #[test]
    fn test_edge_async_functions() {
        let code = r#"
async def async_func():
    await something()

class AsyncClass:
    async def async_method(self):
        pass
"#;
        let (index, stats) = parse_code(code, Language::Python);

        assert_eq!(stats.symbols_found, 3); // async_func, AsyncClass, async_method
    }

    #[test]
    fn test_edge_generator_function() {
        let code = r#"
def generator():
    yield 1
    yield from other_gen()
"#;
        let (index, stats) = parse_code(code, Language::Python);

        assert_eq!(stats.symbols_found, 1);
        assert_eq!(index.symbols()[0].name, "generator");
    }

    #[test]
    fn test_edge_type_annotations() {
        let code = r#"
def typed_func(x: int, y: list[str]) -> dict[str, Any]:
    pass

class TypedClass:
    attr: int = 0
"#;
        let (index, stats) = parse_code(code, Language::Python);

        assert_eq!(stats.symbols_found, 2);
    }

    #[test]
    fn test_edge_string_with_special_chars() {
        let code = r#"
sql = "SELECT * FROM users WHERE name = 'O\'Brien'"
regex = r"\d+\.\d+"
multiline = """
Line 1
Line 2
"""
"#;
        let (index, stats) = parse_code(code, Language::Python);

        assert!(stats.strings_found >= 1);
    }

    #[test]
    fn test_edge_f_string() {
        let code = r#"
name = "world"
msg = f"Hello, {name}!"
"#;
        let (index, _stats) = parse_code(code, Language::Python);

        // Should not crash on f-strings
        assert!(index.strings().len() >= 0);
    }

    #[test]
    fn test_edge_property_decorator() {
        let code = r#"
class WithProperty:
    @property
    def value(self):
        return self._value

    @value.setter
    def value(self, v):
        self._value = v
"#;
        let (index, stats) = parse_code(code, Language::Python);

        assert!(stats.symbols_found >= 2);
    }

    #[test]
    fn test_edge_classmethod_staticmethod() {
        let code = r#"
class Methods:
    @classmethod
    def class_method(cls):
        pass

    @staticmethod
    def static_method():
        pass
"#;
        let (index, stats) = parse_code(code, Language::Python);

        assert_eq!(stats.symbols_found, 3); // class + 2 methods

        let methods: Vec<_> = index
            .symbols()
            .iter()
            .filter(|s| s.kind == SymbolKind::Method)
            .collect();
        assert_eq!(methods.len(), 2);
    }

    #[test]
    fn test_edge_typescript_interface() {
        let code = r#"
interface User {
    name: string;
    age: number;
}

class UserImpl implements User {
    name: string = "";
    age: number = 0;
}
"#;
        let (index, stats) = parse_code(code, Language::TypeScript);

        // Should handle TypeScript interfaces
        assert!(stats.symbols_found >= 1);
    }

    #[test]
    fn test_edge_go_struct_methods() {
        let code = r#"
package main

type Server struct {
    port int
}

func (s *Server) Start() error {
    return nil
}

func (s Server) Stop() {
}
"#;
        let (index, stats) = parse_code(code, Language::Go);

        // Should find struct and methods
        assert!(stats.symbols_found >= 1);
    }

    #[test]
    fn test_edge_java_annotations() {
        let code = r#"
@Service
@Transactional
public class UserService {
    @Autowired
    private UserRepository repo;

    @Override
    public void process() {}
}
"#;
        let (index, stats) = parse_code(code, Language::Java);

        // Should handle Java annotations
        assert!(stats.symbols_found >= 1);
    }

    #[test]
    fn test_edge_rust_impl_trait() {
        let code = r#"
trait Display {
    fn display(&self);
}

struct Point { x: i32, y: i32 }

impl Display for Point {
    fn display(&self) {
        println!("{}, {}", self.x, self.y);
    }
}
"#;
        let (index, stats) = parse_code(code, Language::Rust);

        // Should find struct and impl
        assert!(stats.symbols_found >= 1);
    }

    #[test]
    fn test_edge_rust_macro_invocations() {
        let code = r#"
macro_rules! my_macro {
    () => {};
}

fn main() {
    println!("hello");
    vec![1, 2, 3];
    my_macro!();
}
"#;
        let (index, stats) = parse_code(code, Language::Rust);

        // Should not crash on macros
        assert!(stats.symbols_found >= 1);
    }

    // === Cyclomatic Complexity Tests ===

    #[test]
    fn test_cc_simple_function() {
        let code = r#"
def simple():
    return 1
"#;
        let (_index, stats) = parse_code(code, Language::Python);

        assert_eq!(stats.function_count, 1);
        assert_eq!(stats.total_cc, 1); // Base CC = 1
        assert_eq!(stats.max_function_cc, 1);
    }

    #[test]
    fn test_cc_with_if() {
        let code = r#"
def with_if(x):
    if x > 0:
        return 1
    return 0
"#;
        let (_index, stats) = parse_code(code, Language::Python);

        assert_eq!(stats.function_count, 1);
        assert_eq!(stats.total_cc, 2); // 1 base + 1 if
        assert_eq!(stats.max_function_cc, 2);
    }

    #[test]
    fn test_cc_with_if_elif_else() {
        let code = r#"
def with_branches(x):
    if x > 0:
        return 1
    elif x < 0:
        return -1
    else:
        return 0
"#;
        let (_index, stats) = parse_code(code, Language::Python);

        assert_eq!(stats.function_count, 1);
        assert_eq!(stats.total_cc, 3); // 1 base + 1 if + 1 elif
        assert_eq!(stats.max_function_cc, 3);
    }

    #[test]
    fn test_cc_with_for_loop() {
        let code = r#"
def with_loop(items):
    for item in items:
        print(item)
"#;
        let (_index, stats) = parse_code(code, Language::Python);

        assert_eq!(stats.function_count, 1);
        assert_eq!(stats.total_cc, 2); // 1 base + 1 for
    }

    #[test]
    fn test_cc_with_while_loop() {
        let code = r#"
def with_while(n):
    while n > 0:
        n -= 1
"#;
        let (_index, stats) = parse_code(code, Language::Python);

        assert_eq!(stats.function_count, 1);
        assert_eq!(stats.total_cc, 2); // 1 base + 1 while
    }

    #[test]
    fn test_cc_with_try_except() {
        let code = r#"
def with_try():
    try:
        risky()
    except ValueError:
        handle_value_error()
    except TypeError:
        handle_type_error()
"#;
        let (_index, stats) = parse_code(code, Language::Python);

        assert_eq!(stats.function_count, 1);
        assert_eq!(stats.total_cc, 3); // 1 base + 2 except clauses
    }

    #[test]
    fn test_cc_with_boolean_operators() {
        let code = r#"
def with_bool(a, b, c):
    if a and b:
        return 1
    if a or c:
        return 2
    return 0
"#;
        let (_index, stats) = parse_code(code, Language::Python);

        assert_eq!(stats.function_count, 1);
        // 1 base + 2 if + 2 boolean ops (and, or)
        assert_eq!(stats.total_cc, 5);
    }

    #[test]
    fn test_cc_complex_function() {
        let code = r#"
def complex_func(items, threshold):
    result = []
    for item in items:
        if item > threshold:
            if item > threshold * 2:
                result.append(item * 2)
            else:
                result.append(item)
        elif item == threshold:
            result.append(0)
    return result
"#;
        let (_index, stats) = parse_code(code, Language::Python);

        assert_eq!(stats.function_count, 1);
        // 1 base + 1 for + 2 if + 1 elif = 5
        assert_eq!(stats.total_cc, 5);
    }

    #[test]
    fn test_cc_multiple_functions() {
        let code = r#"
def simple():
    return 1

def with_if(x):
    if x:
        return 1
    return 0

def with_loop(items):
    for i in items:
        if i > 0:
            print(i)
"#;
        let (_index, stats) = parse_code(code, Language::Python);

        assert_eq!(stats.function_count, 3);
        // simple: 1, with_if: 2, with_loop: 3
        assert_eq!(stats.total_cc, 6);
        assert_eq!(stats.max_function_cc, 3);
    }

    #[test]
    fn test_cc_javascript_ternary() {
        let code = r#"
function withTernary(x) {
    return x > 0 ? 1 : -1;
}
"#;
        let (_index, stats) = parse_code(code, Language::JavaScript);

        assert_eq!(stats.function_count, 1);
        // 1 base + 1 ternary
        assert_eq!(stats.total_cc, 2);
    }

    #[test]
    fn test_cc_rust_match_arms() {
        let code = r#"
fn with_match(x: i32) -> i32 {
    match x {
        0 => 0,
        1 => 1,
        _ => 2,
    }
}
"#;
        let (_index, stats) = parse_code(code, Language::Rust);

        assert_eq!(stats.function_count, 1);
        // 1 base + 3 match arms
        assert_eq!(stats.total_cc, 4);
    }

    #[test]
    fn test_cc_go_switch() {
        let code = r#"
package main

func withSwitch(x int) int {
    switch x {
    case 0:
        return 0
    case 1:
        return 1
    default:
        return 2
    }
}
"#;
        let (_index, stats) = parse_code(code, Language::Go);

        assert_eq!(stats.function_count, 1);
        // Go uses expression_switch_statement, not individual cases
        // 1 base + 1 switch statement
        assert!(stats.total_cc >= 2);
    }

    // === Source Text Capture Tests ===

    #[test]
    fn test_function_source_text_captured() {
        let code = r#"
def example_function(arg1, arg2):
    """This is a docstring."""
    result = arg1 + arg2
    return result
"#;
        let (index, _stats) = parse_code(code, Language::Python);

        assert_eq!(index.symbols().len(), 1);
        let func = &index.symbols()[0];
        assert_eq!(func.name, "example_function");

        // source_text should contain the full function
        assert!(func.source_text.contains("def example_function"));
        assert!(func.source_text.contains("This is a docstring"));
        assert!(func.source_text.contains("return result"));
    }

    #[test]
    fn test_class_source_text_captured() {
        let code = r#"
class MyClass:
    """A sample class."""

    def __init__(self, value):
        self.value = value

    def get_value(self):
        return self.value
"#;
        let (index, _stats) = parse_code(code, Language::Python);

        // Should have class + 2 methods
        let class_sym = index.symbols().iter().find(|s| s.name == "MyClass").unwrap();

        // Class source_text should contain entire class definition
        assert!(class_sym.source_text.contains("class MyClass"));
        assert!(class_sym.source_text.contains("A sample class"));
        assert!(class_sym.source_text.contains("def __init__"));
        assert!(class_sym.source_text.contains("def get_value"));
    }

    #[test]
    fn test_method_source_text_captured() {
        let code = r#"
class Container:
    def process(self, data):
        """Process the data."""
        cleaned = data.strip()
        return cleaned.upper()
"#;
        let (index, _stats) = parse_code(code, Language::Python);

        let method = index.symbols().iter().find(|s| s.name == "process").unwrap();

        // Method source_text should contain just the method, not the class
        assert!(method.source_text.contains("def process"));
        assert!(method.source_text.contains("Process the data"));
        assert!(method.source_text.contains("return cleaned.upper()"));
        // Should NOT contain class definition line
        assert!(!method.source_text.contains("class Container"));
    }

    #[test]
    fn test_multiline_function_source_text() {
        let code = r#"
def long_function():
    line1 = 1
    line2 = 2
    line3 = 3
    line4 = 4
    line5 = 5
    return line1 + line2 + line3 + line4 + line5
"#;
        let (index, _stats) = parse_code(code, Language::Python);

        let func = &index.symbols()[0];

        // All lines should be captured
        assert!(func.source_text.contains("line1 = 1"));
        assert!(func.source_text.contains("line5 = 5"));
        assert!(func.source_text.contains("return line1"));
    }

    #[test]
    fn test_rust_function_source_text() {
        let code = r#"
fn calculate(x: i32, y: i32) -> i32 {
    let sum = x + y;
    let product = x * y;
    sum + product
}
"#;
        let (index, _stats) = parse_code(code, Language::Rust);

        let func = &index.symbols()[0];
        assert_eq!(func.name, "calculate");
        assert!(func.source_text.contains("fn calculate"));
        assert!(func.source_text.contains("let sum = x + y"));
        assert!(func.source_text.contains("sum + product"));
    }

    #[test]
    fn test_javascript_function_source_text() {
        let code = r#"
function handleClick(event) {
    event.preventDefault();
    const target = event.target;
    console.log(target);
}
"#;
        let (index, _stats) = parse_code(code, Language::JavaScript);

        let func = &index.symbols()[0];
        assert_eq!(func.name, "handleClick");
        assert!(func.source_text.contains("function handleClick"));
        assert!(func.source_text.contains("event.preventDefault()"));
    }

    #[test]
    fn test_source_text_not_empty() {
        let code = "def minimal(): pass";
        let (index, _stats) = parse_code(code, Language::Python);

        let func = &index.symbols()[0];
        assert!(!func.source_text.is_empty());
        assert!(func.source_text.contains("def minimal"));
    }
}
