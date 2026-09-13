//! Every site as one line, grouped under its file.
//!
//! This is what makes a sweep citable. `census` returns counts, which tell an
//! agent how many sites exist but give it nothing to refer back to. The
//! manifest puts a stable identifier for every candidate into the transcript,
//! so the candidate set stops being something the agent chooses or forgets and
//! becomes text in front of it that it cannot quietly revise.
//!
//! Plain text rather than JSON, deliberately. The agent reads this, it does not
//! parse it, and JSON scaffolding would triple the size for no gain.

use crate::ledger::STATE_LIVE;
use crate::model;

pub fn run(root: &std::path::Path, name: &str, file: Option<&str>, unjudged: bool) {
    let conn = super::open(root, false);
    let (question, _scope) = super::require_sweep(&conn, name);

    let mut query = String::from(
        "SELECT s.file, s.site_id, s.start_line, s.end_line, s.rule_id, v.verdict FROM site s \
         LEFT JOIN verdict v ON v.sweep = s.sweep AND v.site_id = s.site_id \
         WHERE s.sweep = ?1 AND s.state = ?2",
    );
    if file.is_some() {
        query.push_str(" AND s.file = ?3");
    }
    if unjudged {
        query.push_str(" AND v.site_id IS NULL");
    }
    query.push_str(" ORDER BY s.file, s.start_line");

    let mut stmt = conn.prepare(&query).expect("the manifest query compiles");
    let read = |r: &rusqlite::Row<'_>| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, Option<String>>(5)?,
        ))
    };
    let rows: Vec<(String, String, i64, i64, String, Option<String>)> = match file {
        Some(f) => stmt
            .query_map(rusqlite::params![name, STATE_LIVE, f], read)
            .expect("sites read")
            .filter_map(Result::ok)
            .collect(),
        None => stmt
            .query_map(rusqlite::params![name, STATE_LIVE], read)
            .expect("sites read")
            .filter_map(Result::ok)
            .collect(),
    };

    if let Some(f) = file {
        if rows.is_empty() {
            let mut known = conn
                .prepare(
                    "SELECT DISTINCT file FROM site WHERE sweep = ?1 AND state = ?2 \
                     ORDER BY file LIMIT 5",
                )
                .expect("the known-file query compiles");
            let names: Vec<String> = known
                .query_map([name, STATE_LIVE], |r| r.get(0))
                .expect("files read")
                .filter_map(Result::ok)
                .collect();
            super::die(&format!(
                "no sites in '{f}' for sweep '{name}'. Files in this sweep include: {}",
                names.join(", ")
            ));
        }
    }

    let counts = model::coverage(&conn, name);
    let mut rule_ids: Vec<String> = rows.iter().map(|r| r.4.clone()).collect();
    rule_ids.sort();
    rule_ids.dedup();

    let mut out = vec![format!("sweep: {name}"), format!("question: {question}")];
    out.push(format!(
        "rules: {}",
        if rule_ids.is_empty() { "(none matched)".to_string() } else { rule_ids.join(", ") }
    ));
    if let Some(f) = file {
        out.push(format!("filtered to: {f}"));
    }
    if unjudged {
        out.push("filtered to: unjudged only".to_string());
    }
    out.push(format!(
        "listed {} of {} live sites, {} unjudged",
        rows.len(),
        counts.live_sites,
        counts.unjudged
    ));
    out.push(String::new());

    let mut current: Option<String> = None;
    for (file, site_id, start, end, _rule, verdict) in &rows {
        if current.as_deref() != Some(file.as_str()) {
            current = Some(file.clone());
            out.push(file.clone());
        }
        let mark: String = verdict
            .clone()
            .unwrap_or_else(|| "-".to_string())
            .chars()
            .take(9)
            .collect();
        out.push(format!("  {site_id}  {start}-{end}  {mark}"));
    }

    out.push(String::new());
    out.push(
        "Every candidate is listed above. This set is complete with respect to \
         the rules named, and to syntax: matching does no scope or type \
         analysis, so a site of a different shape was never a candidate."
            .to_string(),
    );
    println!("{}", out.join("\n"));
}
