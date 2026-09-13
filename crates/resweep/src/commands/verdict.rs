//! Recording a judgement, and recording an independent second one.

use rusqlite::Connection;
use serde_json::{Map, Value, json};

use crate::ledger::VERDICTS;
use crate::model;
use crate::output;

pub struct Args<'a> {
    pub name: &'a str,
    pub site: Option<&'a str>,
    pub verdict: Option<&'a str>,
    pub note: &'a str,
    pub method: &'a str,
    pub from_json: Option<&'a str>,
    pub second_opinion: bool,
}

pub fn apply(conn: &Connection, name: &str, site_id: &str, verdict: &str, note: &str, method: &str) {
    check(site_id, verdict, note);
    let exists: Option<String> = conn
        .query_row(
            "SELECT state FROM site WHERE sweep = ?1 AND site_id = ?2",
            [name, site_id],
            |r| r.get(0),
        )
        .ok();
    if exists.is_none() {
        super::die(&format!(
            "site {site_id} is not in sweep '{name}'. Site ids come from `next`."
        ));
    }
    conn.execute(
        "INSERT INTO verdict (sweep, site_id, verdict, note, method, judged_at) \
         VALUES (?1,?2,?3,?4,?5,?6) \
         ON CONFLICT(sweep, site_id) DO UPDATE SET \
         verdict = excluded.verdict, note = excluded.note, \
         method = excluded.method, judged_at = excluded.judged_at",
        rusqlite::params![name, site_id, verdict, note.trim(), method.trim(), output::now()],
    )
    .expect("the verdict is written");
    // Re-judging settles a dispute: the operator has now decided, so the second
    // opinion it conflicted with is no longer an open question.
    conn.execute(
        "DELETE FROM second_opinion WHERE sweep = ?1 AND site_id = ?2",
        [name, site_id],
    )
    .expect("any second opinion is cleared");
}

/// An independent second judgement. The first is never touched.
pub fn apply_second_opinion(
    conn: &Connection,
    name: &str,
    site_id: &str,
    verdict: &str,
    note: &str,
    method: &str,
) {
    check(site_id, verdict, note);
    let first: Option<String> = conn
        .query_row(
            "SELECT verdict FROM verdict WHERE sweep = ?1 AND site_id = ?2",
            [name, site_id],
            |r| r.get(0),
        )
        .ok();
    if first.is_none() {
        super::die(&format!(
            "site {site_id} has no first judgement, so there is nothing to compare \
             against. Judge it with `verdict` before rechecking it."
        ));
    }
    let already: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM second_opinion WHERE sweep = ?1 AND site_id = ?2",
            [name, site_id],
            |r| r.get(0),
        )
        .ok();
    if already.is_some() {
        // A third opinion is a different feature. Overwriting the second would
        // quietly destroy the comparison this whole command exists to produce.
        super::die(&format!("site {site_id} already has a second opinion"));
    }
    conn.execute(
        "INSERT INTO second_opinion (sweep, site_id, verdict, note, method, judged_at) \
         VALUES (?1,?2,?3,?4,?5,?6)",
        rusqlite::params![name, site_id, verdict, note.trim(), method.trim(), output::now()],
    )
    .expect("the second opinion is written");
}

fn check(site_id: &str, verdict: &str, note: &str) {
    if !VERDICTS.contains(&verdict) {
        super::die(&format!(
            "verdict must be one of {}, got '{verdict}'",
            VERDICTS.join(", ")
        ));
    }
    // A verdict without a note is not evidence, and accepting one silently
    // would hollow out the ledger while every count kept adding up.
    if note.trim().is_empty() {
        super::die(&format!("site {site_id}: a verdict without a note is not a judgement"));
    }
}

pub fn run(root: &std::path::Path, args: Args<'_>) {
    let conn = super::open(root, false);
    super::require_sweep(&conn, args.name);

    let applied = if let Some(source) = args.from_json {
        let body = if source == "-" {
            let mut buf = String::new();
            use std::io::Read;
            std::io::stdin().read_to_string(&mut buf).ok();
            buf
        } else {
            std::fs::read_to_string(source).unwrap_or_else(|e| super::die(&e.to_string()))
        };
        let batch: Value = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(e) => super::die(&format!("verdict batch is not valid JSON: {e}")),
        };
        let Some(items) = batch.as_array() else {
            super::die("verdict batch must be a JSON array of {site_id, verdict, note}")
        };
        for item in items {
            apply(
                &conn,
                args.name,
                item["site_id"].as_str().unwrap_or_default(),
                item["verdict"].as_str().unwrap_or_default(),
                item["note"].as_str().unwrap_or_default(),
                item["method"].as_str().unwrap_or_default(),
            );
        }
        items.len()
    } else {
        let (Some(site), Some(verdict)) = (args.site, args.verdict) else {
            super::die("give --site and --verdict, or --from-json for a batch")
        };
        if args.second_opinion {
            apply_second_opinion(&conn, args.name, site, verdict, args.note, args.method);
        } else {
            apply(&conn, args.name, site, verdict, args.note, args.method);
        }
        1
    };

    let counts = model::coverage(&conn, args.name);
    let mut payload = Map::new();
    payload.insert("applied".into(), json!(applied));
    payload.insert("coverage".into(), Value::Object(counts.payload));
    println!("{}", output::json(&Value::Object(payload)));
}
