//! The Rust port of resweep, arriving one piece at a time.
//!
//! Kept as a library with a thin binary over it so that every piece can be
//! tested directly, and so an integration test can compare this engine against
//! the one it replaces without going through a command line that does not
//! exist yet.

pub mod commands;
pub mod discovery;
pub mod languages;
pub mod ledger;
pub mod model;
pub mod output;
pub mod resolution;
pub mod rules;
