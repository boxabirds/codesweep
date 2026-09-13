//! The commands, ported so the bytes out are unchanged.
//!
//! Every payload here is built key by key in the order the tool being replaced
//! builds it, and every message is the same words on the same stream with the
//! same exit code. That is not stylistic fidelity: the guidance reads these
//! messages and the suites compare them, so a reworded refusal is a broken
//! contract even when it reads better.

pub mod census;
pub mod list;
pub mod manifest;
pub mod next;
pub mod recheck;
pub mod report;
pub mod show;
pub mod status;
pub mod surfaces;
pub mod verdict;

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::ledger;
use crate::model;

/// Everything a command needs that does not come from its own flags.
pub struct Context {
    pub root: PathBuf,
}

/// Print to standard error with the program's name in front, and stop.
///
/// One exit code for every refusal the tool raises itself, matching the tool
/// being replaced. Argument-parsing refusals exit with two, which is what
/// argparse does and what the recorded reference holds.
pub fn die(message: &str) -> ! {
    eprintln!("resweep: {message}");
    std::process::exit(1)
}

pub fn open(root: &Path, create: bool) -> Connection {
    match ledger::connect(root, create) {
        Ok(conn) => conn,
        Err(e) => die(&e.to_string()),
    }
}

/// The sweep's question and scope, or a refusal naming what does exist.
///
/// Naming the known sweeps is the difference between a dead end and a typo the
/// operator can see.
pub fn require_sweep(conn: &Connection, name: &str) -> (String, String) {
    match model::sweep_row(conn, name) {
        Some(row) => row,
        None => {
            let known = model::known_sweeps(conn);
            die(&format!(
                "no sweep named '{name}'. Known sweeps: {}",
                if known.is_empty() { "(none)".to_string() } else { known.join(", ") }
            ))
        }
    }
}

/// Resolve the root the way the tool being replaced does: textually, never by
/// following symlinks, and relative to the working directory when relative.
pub fn absolute(path: &str) -> PathBuf {
    let p = Path::new(path);
    let joined = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")).join(p)
    };
    let mut out = PathBuf::new();
    for part in joined.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// `os.path.relpath`, for the one place the output holds a path relative to
/// the root.
pub fn relative_to(path: &Path, base: &Path) -> String {
    if let Ok(rest) = path.strip_prefix(base) {
        let s = rest.to_string_lossy().to_string();
        return if s.is_empty() { ".".to_string() } else { s };
    }
    let base_parts: Vec<_> = base.components().collect();
    let path_parts: Vec<_> = path.components().collect();
    let shared = base_parts
        .iter()
        .zip(&path_parts)
        .take_while(|(a, b)| a == b)
        .count();
    let mut out: Vec<String> = vec!["..".to_string(); base_parts.len() - shared];
    out.extend(
        path_parts[shared..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().to_string()),
    );
    if out.is_empty() { ".".to_string() } else { out.join("/") }
}
