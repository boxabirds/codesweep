//! Judged sites offered for an independent second look, first verdict hidden.

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::ledger::STATE_LIVE;
use crate::model;
use crate::output;

/// A shuffle that gives the same sample every run for a given sweep, drawn
/// without regard to verdict.
///
/// Sampling the passes is the tempting choice and would make the disagreement
/// rate a statement about passes rather than about judgements.
///
/// The sequence differs from the Python tool's, which used that language's own
/// generator seeded by the sweep name. Reproducing it would mean carrying an
/// implementation of Mersenne Twister and Python's seeding rules for a
/// property nothing depends on: what the tool promises is that the same sample
/// comes back, not which sample it is. Someone who reruns `recheck` across the
/// upgrade sees a different set offered, and no recorded judgement changes.
fn seeded_shuffle<T>(items: &mut [T], seed: &str) {
    let mut state = Sha256::digest(seed.as_bytes());
    let mut next = |bound: usize| -> usize {
        let mut hasher = Sha256::new();
        hasher.update(state);
        state = hasher.finalize();
        let mut n = 0u64;
        for byte in state.iter().take(8) {
            n = (n << 8) | *byte as u64;
        }
        (n % bound as u64) as usize
    };
    for i in (1..items.len()).rev() {
        items.swap(i, next(i + 1));
    }
}

pub fn run(root: &std::path::Path, name: &str, limit: usize, site: Option<&str>, context: usize) {
    let conn = super::open(root, false);
    super::require_sweep(&conn, name);

    let mut stmt = conn
        .prepare(
            "SELECT s.site_id, s.rule_id, s.file, s.start_line, s.end_line FROM site s \
             JOIN verdict v ON v.sweep = s.sweep AND v.site_id = s.site_id \
             LEFT JOIN second_opinion o ON o.sweep = s.sweep AND o.site_id = s.site_id \
             WHERE s.sweep = ?1 AND s.state = ?2 AND o.site_id IS NULL \
             ORDER BY s.site_id",
        )
        .expect("the recheck query compiles");
    let mut candidates: Vec<(String, String, String, i64, i64)> = stmt
        .query_map([name, STATE_LIVE], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .expect("candidates read")
        .filter_map(Result::ok)
        .collect();

    if let Some(wanted) = site {
        candidates.retain(|c| c.0 == wanted);
        if candidates.is_empty() {
            super::die(&format!(
                "site {wanted} is not available to recheck: it must be a live \
                 site with a first judgement and no second opinion yet"
            ));
        }
    } else if candidates.is_empty() {
        let mut payload = Map::new();
        payload.insert("sweep".into(), json!(name));
        payload.insert("sites".into(), json!([]));
        payload.insert("note".into(), json!(
            "Nothing to recheck. A second look needs a site that has already \
             been judged and not yet rechecked."
        ));
        println!("{}", output::json(&Value::Object(payload)));
        return;
    } else {
        seeded_shuffle(&mut candidates, name);
        candidates.truncate(limit);
    }

    // Nothing of the first judgement appears here. Not a field, not the note,
    // not the method. A reviewer who can see the first verdict is measuring
    // their own deference rather than judging.
    let mut payload = Map::new();
    payload.insert("sweep".into(), json!(name));
    payload.insert("instruction".into(), json!(
        "Judge each site as though for the first time. The earlier verdict is \
         deliberately not shown. Record with \
         `verdict <sweep> --second-opinion --site <id> --verdict <v> --note <why>`."
    ));
    payload.insert(
        "sites".into(),
        Value::Array(
            candidates
                .into_iter()
                .map(|(site_id, rule_id, file, start, end)| {
                    let mut m = Map::new();
                    m.insert("site_id".into(), json!(site_id));
                    m.insert("rule_id".into(), json!(rule_id));
                    m.insert("file".into(), json!(file));
                    m.insert("lines".into(), json!(format!("{start}-{end}")));
                    m.insert(
                        "context".into(),
                        json!(model::read_context(root, &file, start as usize, end as usize, context)),
                    );
                    Value::Object(m)
                })
                .collect(),
        ),
    );
    println!("{}", output::json(&Value::Object(payload)));
}
