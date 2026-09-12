//! scanner-core: Fast codebase indexing and querying via tree-sitter.
//!
//! This crate provides:
//! - Multi-language parsing (Python, JavaScript, TypeScript, Go, Rust, Java)
//! - Symbol extraction (classes, functions, methods)
//! - Inheritance and call graph analysis
//! - String literal and import tracking
//! - Fast query interface via inverted indices
//!
//! For Python bindings, see the `scanner-py` crate.

mod index;
mod languages;
mod parser;
mod queries;

pub use index::{
    CallSite, Import, IndexStats, Inheritance, Location, StringLiteral, Symbol, SymbolIndex,
    SymbolKind,
};
pub use languages::Language;
pub use parser::{parse_file, ParseError, ParseStats};
pub use queries::read_file_section;
