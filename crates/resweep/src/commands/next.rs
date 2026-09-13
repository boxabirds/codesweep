//! The next batch of unjudged sites, with enough surrounding source to judge.

use serde_json::{Map, Value, json};

use crate::ledger::STATE_LIVE;
use crate::model;
use crate::output;

pub fn run(root: &std::path::Path, name: &str, limit: usize, context: usize) {
    let conn = super::open(root, false);
    let (question, _scope) = super::require_sweep(&conn, name);
    let mut stmt = conn
        .prepare(
            "SELECT s.site_id, s.rule_id, s.file, s.start_line, s.end_line FROM site s \
             LEFT JOIN verdict v ON v.sweep = s.sweep AND v.site_id = s.site_id \
             WHERE s.sweep = ?1 AND s.state = ?2 AND v.site_id IS NULL \
             ORDER BY s.file, s.start_line LIMIT ?3",
        )
        .expect("the next query compiles");
    let rows: Vec<(String, String, String, i64, i64)> = stmt
        .query_map(rusqlite::params![name, STATE_LIVE, limit as i64], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .expect("sites read")
        .filter_map(Result::ok)
        .collect();

    let counts = model::coverage(&conn, name);
    let mut payload = Map::new();
    payload.insert("sweep".into(), json!(name));
    payload.insert("question".into(), json!(question));
    payload.insert(
        "remaining_after_this_batch".into(),
        json!(std::cmp::max(0, counts.unjudged - rows.len() as i64)),
    );
    payload.insert(
        "sites".into(),
        Value::Array(
            rows.into_iter()
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
