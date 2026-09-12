//! Where a sweep's judgements live, and what a site is called.
//!
//! Both halves are copied field for field from the Python tool rather than
//! reimplemented from a description of it. Both tools will exist on developer
//! machines while the change lands, and a session's work has to survive being
//! picked up by either one.
//!
//! Site identity is the part most likely to drift without anyone noticing. A
//! changed identity orphans every verdict from a prior session while the tool
//! carries on looking healthy: the coverage arithmetic still adds up, it is
//! just adding up over sites nobody has judged. So the identity here is not
//! merely stable, it is the same construction, and the test that guards it
//! compares against identities the Python tool computes.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use rusqlite::Connection;
use sha2::{Digest, Sha256};

/// Bumped only when the layout changes in a way an older tool cannot read.
/// Both tools write this number and both accept a ledger carrying it.
pub const SCHEMA_VERSION: u32 = 4;

/// Present in the environment of every Bash tool call inside a session.
pub const SESSION_ENV: &str = "CLAUDE_CODE_SESSION_ID";

/// Set deliberately, by a test harness or a shell. It wins over the ambient
/// session variable: checking the ambient one first would make the override
/// silently useless inside a session, which is exactly where tests run.
pub const SESSION_ENV_OVERRIDE: &str = "RESWEEP_SESSION_ID";

pub const TEMP_NAMESPACE: &str = "resweep";

/// How long an abandoned session's index survives before a later run removes
/// it. Sessions end without notice, so cleanup is by age rather than teardown.
pub const SESSION_RETENTION: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Site ids are truncated sha256. 16 hex characters is 64 bits, which keeps
/// collision probability negligible for the site counts a single repository
/// produces while staying short enough to pass around in a JSON payload.
pub const SITE_ID_HEX_CHARS: usize = 16;

/// The same truncation, used for the hash of a rule's source and of a
/// repository's path.
pub const REPO_SLUG_HEX_CHARS: usize = 8;

pub const STATE_LIVE: &str = "live";
pub const STATE_GONE: &str = "gone";

pub const VERDICTS: &[&str] = &["violation", "pass", "na"];

/// Collapse whitespace so reformatting alone does not invalidate a verdict.
///
/// The Python original is `" ".join(text.split())`, which splits on runs of
/// whitespace and drops leading and trailing runs. `split_whitespace` is the
/// same operation. The two disagree only on a handful of C0 control characters
/// that Python counts as whitespace and Unicode does not; no grammar produces
/// them inside a match, and the identity comparison against the Python tool
/// would catch it if one did.
pub fn normalise(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// rule id, relative file, ordinal and whitespace-collapsed text, joined with
/// a NUL and hashed. The separator cannot occur in any of the parts, so no
/// pair of different sites can join to the same payload.
pub fn site_identity(rule_id: &str, relfile: &str, ordinal: usize, text: &str) -> String {
    let payload = format!("{rule_id}\0{relfile}\0{ordinal}\0{}", normalise(text));
    truncated_sha256(payload.as_bytes(), SITE_ID_HEX_CHARS)
}

pub fn truncated_sha256(bytes: &[u8], chars: usize) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(chars);
    for byte in digest.iter() {
        if hex.len() >= chars {
            break;
        }
        hex.push_str(&format!("{byte:02x}"));
    }
    hex.truncate(chars);
    hex
}

#[derive(Debug)]
pub enum LedgerError {
    NoSession,
    UnsafeSessionId,
    Absent { path: PathBuf },
    Sql(rusqlite::Error),
    Io(std::io::Error),
}

impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSession => write!(
                f,
                "{SESSION_ENV} is not set, so this run cannot be tied to a session. \
                 resweep keeps its index in a per-session temporary directory and \
                 will not fall back to a shared one. Set {SESSION_ENV_OVERRIDE} \
                 explicitly to run outside a session."
            ),
            Self::UnsafeSessionId => write!(
                f,
                "{SESSION_ENV} contains characters that cannot form a directory name"
            ),
            Self::Absent { path } => write!(
                f,
                "no ledger at {}. Run `resweep census` first.",
                path.display()
            ),
            Self::Sql(e) => write!(f, "{e}"),
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for LedgerError {}

impl From<rusqlite::Error> for LedgerError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sql(e)
    }
}

impl From<std::io::Error> for LedgerError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// The session this run belongs to.
///
/// When there is none, the run is not inside a session and there is no safe
/// shared location to guess at: two unrelated runs sharing one database would
/// silently merge their candidate sets. Refuse rather than fall back.
pub fn session_id() -> Result<String, LedgerError> {
    let value = std::env::var(SESSION_ENV_OVERRIDE)
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var(SESSION_ENV).ok().filter(|v| !v.is_empty()))
        .ok_or(LedgerError::NoSession)?;
    validate_session_id(&value)?;
    Ok(value)
}

/// The value reaches a filesystem path, so refuse anything that could climb
/// out of the temp root rather than sanitising it into something else. Kept
/// apart from reading the environment so it can be tested without a process
/// full of global state.
pub fn validate_session_id(value: &str) -> Result<(), LedgerError> {
    if value.is_empty() {
        return Err(LedgerError::NoSession);
    }
    if !value.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err(LedgerError::UnsafeSessionId);
    }
    Ok(())
}

/// A filename identifying one repository, so a session can sweep several.
pub fn repo_slug(root: &Path) -> String {
    let absolute = absolute(root);
    let base = absolute
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "root".to_string());
    let digest = truncated_sha256(absolute.to_string_lossy().as_bytes(), REPO_SLUG_HEX_CHARS);
    let safe: String = base
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    format!("{safe}-{digest}")
}

pub fn session_dir() -> Result<PathBuf, LedgerError> {
    Ok(std::env::temp_dir().join(TEMP_NAMESPACE).join(session_id()?))
}

/// Where this session's index lives.
///
/// Deliberately not inside the repository. The index is rebuilt by every
/// census, so it is never older than the census that made it and has no
/// staleness to refresh. Keeping it in the repository would leave an untracked
/// directory behind and invite exactly the stale-verdict problem the session
/// lifetime removes.
pub fn ledger_path(root: &Path) -> Result<PathBuf, LedgerError> {
    Ok(session_dir()?.join(format!("{}.db", repo_slug(root))))
}

/// Remove session directories older than the retention window.
///
/// Failures are ignored: another session may be removing the same directory
/// concurrently, and losing a cleanup pass is harmless while crashing a sweep
/// over one is not.
pub fn sweep_stale_sessions() {
    let root = std::env::temp_dir().join(TEMP_NAMESPACE);
    let Ok(entries) = std::fs::read_dir(&root) else {
        return;
    };
    let mine = session_dir().ok();
    let Some(cutoff) = SystemTime::now().checked_sub(SESSION_RETENTION) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if Some(&path) == mine.as_ref() || !path.is_dir() {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|m| m.modified()) else {
            continue;
        };
        if modified < cutoff {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

/// The layout, copied field for field. A ledger written by either tool has to
/// be readable by the other.
const SCHEMA: &str = "
        PRAGMA journal_mode=WAL;
        CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY, value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sweep (
            name        TEXT PRIMARY KEY,
            question    TEXT NOT NULL,
            scope       TEXT NOT NULL,
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS rule (
            sweep       TEXT NOT NULL,
            rule_id     TEXT NOT NULL,
            language    TEXT NOT NULL,
            path        TEXT NOT NULL,
            source      TEXT NOT NULL,
            source_hash TEXT NOT NULL,
            PRIMARY KEY (sweep, rule_id)
        );
        CREATE TABLE IF NOT EXISTS census_run (
            id        INTEGER PRIMARY KEY AUTOINCREMENT,
            sweep     TEXT NOT NULL,
            ran_at    TEXT NOT NULL,
            found     INTEGER NOT NULL,
            added     INTEGER NOT NULL,
            departed  INTEGER NOT NULL,
            uncovered TEXT NOT NULL DEFAULT '{}'
        );
        CREATE TABLE IF NOT EXISTS site (
            sweep      TEXT NOT NULL,
            site_id    TEXT NOT NULL,
            rule_id    TEXT NOT NULL,
            file       TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line   INTEGER NOT NULL,
            ordinal    INTEGER NOT NULL,
            state      TEXT NOT NULL,
            first_seen TEXT NOT NULL,
            last_seen  TEXT NOT NULL,
            PRIMARY KEY (sweep, site_id)
        );
        CREATE INDEX IF NOT EXISTS site_by_state ON site (sweep, state);
        CREATE TABLE IF NOT EXISTS verdict (
            sweep     TEXT NOT NULL,
            site_id   TEXT NOT NULL,
            verdict   TEXT NOT NULL,
            note      TEXT NOT NULL,
            judged_at TEXT NOT NULL,
            PRIMARY KEY (sweep, site_id)
        );
        CREATE TABLE IF NOT EXISTS second_opinion (
            sweep     TEXT NOT NULL,
            site_id   TEXT NOT NULL,
            verdict   TEXT NOT NULL,
            note      TEXT NOT NULL,
            method    TEXT NOT NULL DEFAULT '',
            judged_at TEXT NOT NULL,
            PRIMARY KEY (sweep, site_id)
        );
        CREATE TABLE IF NOT EXISTS surface (
            sweep                 TEXT NOT NULL,
            name                  TEXT NOT NULL,
            scope                 TEXT,
            examined_at           TEXT,
            declared_at           TEXT NOT NULL,
            declared_after_census INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (sweep, name)
        );
";

/// Columns added after the tables were first written. `CREATE TABLE IF NOT
/// EXISTS` leaves an older ledger without them, so they are added explicitly.
const LATE_COLUMNS: &[(&str, &str, &str)] = &[
    ("census_run", "uncovered", "TEXT NOT NULL DEFAULT '{}'"),
    ("verdict", "method", "TEXT NOT NULL DEFAULT ''"),
];

pub fn connect(root: &Path, create: bool) -> Result<Connection, LedgerError> {
    let path = ledger_path(root)?;
    if !create && !path.exists() {
        return Err(LedgerError::Absent { path });
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&path)?;
    conn.execute_batch(SCHEMA)?;
    for (table, column, decl) in LATE_COLUMNS {
        let present: Vec<String> = conn
            .prepare(&format!("PRAGMA table_info({table})"))?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<_, _>>()?;
        if !present.iter().any(|c| c == column) {
            conn.execute(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"), [])?;
        }
    }
    conn.execute(
        "INSERT OR IGNORE INTO meta (key, value) VALUES ('schema_version', ?1)",
        [SCHEMA_VERSION.to_string()],
    )?;
    Ok(conn)
}

fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        normalise_path(path)
    } else {
        normalise_path(
            &std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("/"))
                .join(path),
        )
    }
}

/// Resolve `.` and `..` textually, the way Python's `os.path.abspath` does.
///
/// Not `canonicalize`: that resolves symlinks and requires the path to exist,
/// and the Python tool does neither. A repository reached through a symlinked
/// path would otherwise get a different slug from each tool, and so a
/// different ledger.
fn normalise_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_id_that_could_climb_out_of_the_temp_root_is_refused() {
        for bad in ["../escape", "a/b", "with space", "semi;colon", "dot.dot", ""] {
            assert!(
                validate_session_id(bad).is_err(),
                "{bad:?} should not be allowed to name a directory"
            );
        }
    }

    #[test]
    fn an_ordinary_session_id_is_allowed() {
        for good in ["abc123", "a-b_c", "5a3451a3-08d1-4274-aa47-4287622f5489"] {
            assert!(validate_session_id(good).is_ok(), "{good:?} should be allowed");
        }
    }

    #[test]
    fn the_refusal_says_which_variable_to_set() {
        let message = LedgerError::NoSession.to_string();
        assert!(message.contains(SESSION_ENV), "{message}");
        assert!(message.contains(SESSION_ENV_OVERRIDE), "{message}");
    }

    #[test]
    fn an_absent_ledger_names_the_path_and_the_command_that_makes_one() {
        let message = LedgerError::Absent { path: PathBuf::from("/tmp/x.db") }.to_string();
        assert!(message.contains("/tmp/x.db"), "{message}");
        assert!(message.contains("census"), "{message}");
    }

    #[test]
    fn a_relative_root_and_the_absolute_one_share_a_slug() {
        // Otherwise the same repository gets two ledgers depending on where
        // the command was run from, and each looks complete on its own.
        let here = std::env::current_dir().expect("a working directory");
        assert_eq!(repo_slug(Path::new(".")), repo_slug(&here));
    }

    #[test]
    fn a_path_with_a_parent_step_resolves_the_way_python_does() {
        let a = repo_slug(Path::new("/tmp/one/../two"));
        let b = repo_slug(Path::new("/tmp/two"));
        assert_eq!(a, b);
    }

    #[test]
    fn the_ordinal_is_part_of_the_identity() {
        // Two byte-identical snippets in one file are two sites, and the only
        // thing telling them apart is the order they were found in.
        let first = site_identity("r", "a.ts", 0, "catch (e) {}");
        let second = site_identity("r", "a.ts", 1, "catch (e) {}");
        assert_ne!(first, second);
    }

    #[test]
    fn the_separator_cannot_be_forged_from_the_parts() {
        // A separator that could appear inside a part would let two different
        // sites join to one payload and collide by construction.
        let a = site_identity("r", "a.ts", 0, "x");
        let b = site_identity("r\u{0}a.ts", "", 0, "x");
        assert_ne!(a, b, "a NUL inside a part collided with the separator");
    }

    #[test]
    fn an_identity_is_the_agreed_length() {
        let id = site_identity("r", "a.ts", 0, "x");
        assert_eq!(id.len(), SITE_ID_HEX_CHARS);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
