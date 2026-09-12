//! Which files are in scope, and by which rules.
//!
//! This replaces asking version control for a file list. The replacement is
//! not neutral: the two can disagree, and the file set decides the denominator
//! of every count the tool prints. A quiet difference here invalidates all of
//! them without changing how any of them look, so the method is reported and
//! every known disagreement is written down rather than absorbed.

use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

/// The ignore sources that were actually in effect, so the report can say
/// which rules were applied rather than which program was asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// At least one ignore file governed the walk.
    IgnoreFiles,
    /// Nothing to obey, so every file under the scope is in.
    Walk,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IgnoreFiles => "ignore-files",
            Self::Walk => "walk",
        }
    }
}

#[derive(Debug)]
pub struct Discovery {
    /// Paths relative to the scope, in the order the walk produced them.
    pub files: Vec<String>,
    pub method: Method,
}

/// Directories never worth walking, whatever the ignore files say.
///
/// Only `.git` is unconditional in the walker itself. The rest are here for a
/// scope with no ignore files at all, where the old tool's hardcoded list was
/// the only thing keeping a vendored bundle out of the count.
pub const UNWALKED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    "vendor",
    ".venv",
    "venv",
    "__pycache__",
    ".next",
    ".cache",
    "coverage",
];

/// A relative scope resolves against the root, never against the working
/// directory. Resolving against the working directory would let the same
/// command sweep a different tree depending on where it was run, and still
/// print its coverage arithmetic with full confidence.
pub fn resolve_scope(root: &Path, scope: Option<&str>) -> PathBuf {
    match scope {
        None => root.to_path_buf(),
        Some(s) => {
            let p = Path::new(s);
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                root.join(p)
            }
        }
    }
}

/// Whether any ignore file governs this scope.
fn ignore_files_present(scope: &Path) -> bool {
    // Walk up as well as down: a .gitignore at the repository root governs a
    // scope several directories inside it.
    let mut here = Some(scope);
    while let Some(dir) = here {
        for name in [".gitignore", ".ignore", ".rgignore"] {
            if dir.join(name).is_file() {
                return true;
            }
        }
        if dir.join(".git").join("info").join("exclude").is_file() {
            return true;
        }
        here = dir.parent();
    }
    // A nested one, below the scope, counts too.
    WalkBuilder::new(scope)
        .hidden(false)
        .standard_filters(false)
        .build()
        .filter_map(Result::ok)
        .any(|e| {
            e.file_name() == ".gitignore" || e.file_name() == ".ignore"
        })
}

pub fn discover(scope: &Path) -> Discovery {
    let method = if ignore_files_present(scope) {
        Method::IgnoreFiles
    } else {
        Method::Walk
    };

    let mut builder = WalkBuilder::new(scope);
    builder
        // Dotfiles are ordinary source in plenty of repositories and version
        // control lists them, so they are not hidden from the census either.
        .hidden(false)
        .parents(true)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true)
        .ignore(true)
        .require_git(false);
    builder.filter_entry(|entry| {
        let name = entry.file_name().to_string_lossy();
        !(entry.file_type().is_some_and(|t| t.is_dir()) && UNWALKED_DIRS.contains(&name.as_ref()))
    });

    let mut files: Vec<String> = builder
        .build()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
        .filter_map(|e| {
            e.path()
                .strip_prefix(scope)
                .ok()
                .map(|p| p.to_string_lossy().to_string())
        })
        .collect();
    // The walker interleaves directories, so the order is not stable between
    // runs. Every count the tool prints is a set operation, but the order
    // reaches the output in the surface proposal, so it is fixed here.
    files.sort();
    Discovery { files, method }
}
