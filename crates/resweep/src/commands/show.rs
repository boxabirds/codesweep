//! Re-read one site after it has left the `next` queue.
//!
//! Revisiting a verdict without this means judging blind from memory, which is
//! the habit the ledger exists to replace.

use serde_json::{Map, Value, json};

use crate::model;
use crate::output;

pub fn run(root: &std::path::Path, name: &str, site: &str, context: usize) {
    let conn = super::open(root, false);
    super::require_sweep(&conn, name);
    let row = conn
        .query_row(
            "SELECT site_id, rule_id, file, start_line, end_line, state \
             FROM site WHERE sweep = ?1 AND site_id = ?2",
            [name, site],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, String>(5)?,
                ))
            },
        )
        .ok();
    let Some((site_id, rule_id, file, start, end, state)) = row else {
        super::die(&format!("site {site} is not in sweep '{name}'"))
    };

    let verdict = conn
        .query_row(
            "SELECT verdict, note, judged_at FROM verdict WHERE sweep = ?1 AND site_id = ?2",
            [name, site],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )
        .ok()
        .map(|(v, note, judged_at)| {
            let mut m = Map::new();
            m.insert("verdict".into(), json!(v));
            m.insert("note".into(), json!(note));
            m.insert("judged_at".into(), json!(judged_at));
            Value::Object(m)
        })
        .unwrap_or(Value::Null);

    let mut payload = Map::new();
    payload.insert("site_id".into(), json!(site_id));
    payload.insert("rule_id".into(), json!(rule_id));
    payload.insert("file".into(), json!(file));
    payload.insert("lines".into(), json!(format!("{start}-{end}")));
    payload.insert("state".into(), json!(state));
    payload.insert("verdict".into(), verdict);
    payload.insert(
        "context".into(),
        json!(model::read_context(root, &file, start as usize, end as usize, context)),
    );
    println!("{}", output::json(&Value::Object(payload)));
}
