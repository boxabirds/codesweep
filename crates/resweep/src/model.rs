//! The arithmetic every command reports, in one place.
//!
//! Coverage, the completeness verdict, the surface counts and the recheck
//! counts are each read by several commands. They lived in one place in the
//! tool being replaced for the reason they live in one place here: two callers
//! computing the same claim separately will eventually disagree, and the
//! disagreement will be invisible because both look like arithmetic.

use rusqlite::Connection;
use serde_json::{Map, Value, json};

use crate::ledger::STATE_LIVE;

pub fn sweep_row(conn: &Connection, name: &str) -> Option<(String, String)> {
    conn.query_row(
        "SELECT question, scope FROM sweep WHERE name = ?1",
        [name],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .ok()
}

pub fn known_sweeps(conn: &Connection) -> Vec<String> {
    conn.prepare("SELECT name FROM sweep ORDER BY name")
        .and_then(|mut s| {
            s.query_map([], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap_or_default()
}

pub struct Surface {
    pub name: String,
    pub scope: Option<String>,
    pub examined_at: Option<String>,
    pub declared_after_census: i64,
}

pub fn surfaces_of(conn: &Connection, sweep: &str) -> Vec<Surface> {
    let mut stmt = conn
        .prepare(
            "SELECT name, scope, examined_at, declared_after_census \
             FROM surface WHERE sweep = ?1 ORDER BY name",
        )
        .expect("the surface table exists");
    let rows = stmt
        .query_map([sweep], |r| {
            Ok(Surface {
                name: r.get(0)?,
                scope: r.get(1)?,
                examined_at: r.get(2)?,
                declared_after_census: r.get(3)?,
            })
        })
        .expect("surfaces read");
    rows.filter_map(Result::ok).collect()
}

/// Which pass we are on, used to mark a surface as a late addition. Zero means
/// no census has run, so the surface was declared up front.
pub fn latest_census_id(conn: &Connection, sweep: &str) -> i64 {
    conn.query_row(
        "SELECT MAX(id) FROM census_run WHERE sweep = ?1",
        [sweep],
        |r| r.get::<_, Option<i64>>(0),
    )
    .ok()
    .flatten()
    .unwrap_or(0)
}

/// Declared, examined, and the names still outstanding.
///
/// `declared_list` distinguishes a list that exists and is fully examined from
/// no list at all. Both produce zero outstanding, and if a caller cannot tell
/// them apart the absence of a list reads as completeness, which is the exact
/// failure this exists to remove.
pub fn surface_counts(conn: &Connection, sweep: &str) -> Vec<(String, Value)> {
    let rows = surfaces_of(conn, sweep);
    let unexamined: Vec<String> = rows
        .iter()
        .filter(|r| r.examined_at.is_none())
        .map(|r| r.name.clone())
        .collect();
    // Late means added after further searching, not merely after the first
    // census. A sweep does not exist until a census creates it, so the earliest
    // possible declaration already follows one; treating that as late would
    // mark every surface late and destroy the signal. The baseline is the pass
    // the list was first written down on, and anything declared after a later
    // pass grew the list.
    let baseline = rows.iter().map(|r| r.declared_after_census).min().unwrap_or(0);
    let late: Vec<String> = rows
        .iter()
        .filter(|r| r.declared_after_census > baseline)
        .map(|r| r.name.clone())
        .collect();
    vec![
        ("declared_list".into(), json!(!rows.is_empty())),
        ("surfaces_declared".into(), json!(rows.len())),
        ("surfaces_examined".into(), json!(rows.len() - unexamined.len())),
        ("surfaces_unexamined".into(), json!(unexamined)),
        ("surfaces_added_late".into(), json!(late)),
    ]
}

/// How many judgements were looked at twice, and how often the two differed.
///
/// A disagreement is a differing verdict, not a differing note. Two people can
/// describe the same conclusion differently, and counting that would make the
/// figure a measure of prose.
pub fn recheck_counts(conn: &Connection, name: &str) -> Vec<(String, Value)> {
    let mut stmt = conn
        .prepare(
            "SELECT v.site_id, v.verdict v1, s.verdict v2 FROM verdict v \
             JOIN second_opinion s ON s.sweep = v.sweep AND s.site_id = v.site_id \
             JOIN site t ON t.sweep = v.sweep AND t.site_id = v.site_id \
             WHERE v.sweep = ?1 AND t.state = ?2",
        )
        .expect("the recheck query compiles");
    let rows: Vec<(String, String, String)> = stmt
        .query_map([name, STATE_LIVE], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .expect("rechecks read")
        .filter_map(Result::ok)
        .collect();
    let disagreed: Vec<String> = rows
        .iter()
        .filter(|(_, a, b)| a != b)
        .map(|(id, _, _)| id.clone())
        .collect();

    let mut stmt = conn
        .prepare(
            "SELECT method FROM verdict WHERE sweep = ?1 \
             UNION SELECT method FROM second_opinion WHERE sweep = ?2",
        )
        .expect("the method query compiles");
    let mut methods: Vec<String> = stmt
        .query_map([name, name], |r| r.get::<_, String>(0))
        .expect("methods read")
        .filter_map(Result::ok)
        .map(|m| m.trim().to_lowercase())
        .filter(|m| !m.is_empty())
        .collect();
    methods.sort();
    methods.dedup();

    vec![
        ("rechecked".into(), json!(rows.len())),
        ("disagreed".into(), json!(disagreed.len())),
        ("disagreeing_sites".into(), json!(disagreed)),
        ("methods_used".into(), json!(methods)),
        // One method applied twice reproduces its blind spots exactly, so a
        // sweep built entirely one way is a weaker claim than one reproduced
        // differently. No method recorded at all is weaker still, not stronger.
        ("single_method".into(), json!(methods.len() <= 1)),
    ]
}

pub struct Coverage {
    pub live_sites: i64,
    pub judged: i64,
    pub unjudged: i64,
    pub rechecked: i64,
    pub single_method: bool,
    pub declared_list: bool,
    pub surfaces_unexamined: Vec<String>,
    pub payload: Map<String, Value>,
}

pub fn coverage(conn: &Connection, name: &str) -> Coverage {
    let live: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM site WHERE sweep = ?1 AND state = ?2",
            [name, STATE_LIVE],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let judged: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM site s JOIN verdict v \
             ON v.sweep = s.sweep AND v.site_id = s.site_id \
             WHERE s.sweep = ?1 AND s.state = ?2",
            [name, STATE_LIVE],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let mut by_verdict = Map::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT v.verdict, COUNT(*) c FROM verdict v JOIN site s \
         ON v.sweep = s.sweep AND v.site_id = s.site_id \
         WHERE s.sweep = ?1 AND s.state = ?2 GROUP BY v.verdict",
    ) {
        if let Ok(rows) = stmt.query_map([name, STATE_LIVE], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        }) {
            for (verdict, count) in rows.flatten() {
                by_verdict.insert(verdict, json!(count));
            }
        }
    }

    let mut payload = Map::new();
    payload.insert("live_sites".into(), json!(live));
    payload.insert("judged".into(), json!(judged));
    payload.insert("unjudged".into(), json!(live - judged));
    payload.insert("by_verdict".into(), Value::Object(by_verdict));

    let surfaces = surface_counts(conn, name);
    let rechecks = recheck_counts(conn, name);
    let declared_list = surfaces
        .iter()
        .find(|(k, _)| k == "declared_list")
        .map(|(_, v)| v.as_bool().unwrap_or(false))
        .unwrap_or(false);
    let surfaces_unexamined: Vec<String> = surfaces
        .iter()
        .find(|(k, _)| k == "surfaces_unexamined")
        .and_then(|(_, v)| v.as_array().cloned())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let rechecked = rechecks
        .iter()
        .find(|(k, _)| k == "rechecked")
        .and_then(|(_, v)| v.as_i64())
        .unwrap_or(0);
    let single_method = rechecks
        .iter()
        .find(|(k, _)| k == "single_method")
        .and_then(|(_, v)| v.as_bool())
        .unwrap_or(true);
    for (key, value) in surfaces.into_iter().chain(rechecks) {
        payload.insert(key, value);
    }

    Coverage {
        live_sites: live,
        judged,
        unjudged: live - judged,
        rechecked,
        single_method,
        declared_list,
        surfaces_unexamined,
        payload,
    }
}

/// The single verdict, and every reason it is false.
///
/// Every reason is returned, not the first. An implementation that stops at
/// the first failure hands the operator one reason, they fix it, and they walk
/// into the next one.
pub fn completeness(counts: &Coverage, uncovered: &Map<String, Value>) -> (bool, Vec<String>) {
    let mut reasons = Vec::new();
    if counts.live_sites == 0 {
        reasons.push("no sites were enumerated, so there is nothing to be complete about".to_string());
    }
    if counts.unjudged != 0 {
        reasons.push(format!("{} enumerated sites have no verdict", counts.unjudged));
    }
    if !uncovered.is_empty() {
        let mut kinds: Vec<&String> = uncovered.keys().collect();
        kinds.sort();
        let kinds: Vec<&str> = kinds.into_iter().map(|s| s.as_str()).collect();
        reasons.push(format!(
            "the scope holds source no rule can reach ({}), so those files were never examined",
            kinds.join(", ")
        ));
    }
    if !counts.surfaces_unexamined.is_empty() {
        reasons.push(format!(
            "declared parts of the system have not been examined: {}",
            counts.surfaces_unexamined.join(", ")
        ));
    }
    (reasons.is_empty(), reasons)
}

/// Lines of surrounding source handed to the agent with each site to judge.
pub const DEFAULT_CONTEXT_LINES: usize = 8;

/// Sites handed out per `next` call. Small enough that a batch of judgements
/// fits comfortably in one agent turn alongside the reasoning for each.
pub const DEFAULT_BATCH_SIZE: usize = 15;

pub fn read_context(root: &std::path::Path, relfile: &str, start: usize, end: usize, context: usize) -> String {
    let path = root.join(relfile);
    let body = match std::fs::read(&path) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
        Err(e) => return format!("<could not read {relfile}: {e}>"),
    };
    // Split the way Python's readlines does: a trailing newline does not make
    // an extra empty line, but a blank line in the middle is a line.
    let mut lines: Vec<&str> = body.split('\n').collect();
    if body.ends_with('\n') {
        lines.pop();
    }
    let lo = start.saturating_sub(1 + context);
    let hi = std::cmp::min(lines.len(), end + context);
    let mut out = Vec::new();
    for i in lo..hi {
        let marker = if start <= i + 1 && i + 1 <= end { ">" } else { " " };
        out.push(format!("{marker}{:6} | {}", i + 1, lines[i].trim_end()));
    }
    out.join("\n")
}
