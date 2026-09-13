//! Coverage arithmetic for one sweep, with every reason it is not complete.

use serde_json::{Map, Value, json};

use crate::model;
use crate::output;

pub fn run(root: &std::path::Path, name: &str) {
    let conn = super::open(root, false);
    let (question, scope) = super::require_sweep(&conn, name);
    let counts = model::coverage(&conn, name);

    let mut stmt = conn
        .prepare(
            "SELECT rule_id, language, path, source_hash FROM rule WHERE sweep = ?1 \
             ORDER BY rule_id",
        )
        .expect("the rule query compiles");
    let rules: Vec<Value> = stmt
        .query_map([name], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .expect("rules read")
        .filter_map(Result::ok)
        .map(|(rule_id, language, path, source_hash)| {
            let mut m = Map::new();
            m.insert("rule_id".into(), json!(rule_id));
            m.insert("language".into(), json!(language));
            m.insert("path".into(), json!(path));
            m.insert("source_hash".into(), json!(source_hash));
            Value::Object(m)
        })
        .collect();

    let last = conn
        .query_row(
            "SELECT ran_at, found, added, departed, uncovered FROM census_run \
             WHERE sweep = ?1 ORDER BY id DESC LIMIT 1",
            [name],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, String>(4)?,
                ))
            },
        )
        .ok();

    let uncovered: Map<String, Value> = last
        .as_ref()
        .and_then(|(_, _, _, _, raw)| serde_json::from_str(raw).ok())
        .unwrap_or_default();
    let (complete, reasons) = model::completeness(&counts, &uncovered);

    let last_census = match &last {
        None => Value::Null,
        Some((ran_at, found, added, departed, raw)) => {
            let mut m = Map::new();
            m.insert("ran_at".into(), json!(ran_at));
            m.insert("found".into(), json!(found));
            m.insert("added".into(), json!(added));
            m.insert("departed".into(), json!(departed));
            m.insert("uncovered".into(), json!(raw));
            Value::Object(m)
        }
    };

    let mut payload = Map::new();
    payload.insert("sweep".into(), json!(name));
    payload.insert("question".into(), json!(question));
    payload.insert("scope".into(), json!(scope));
    payload.insert("rules".into(), Value::Array(rules));
    payload.insert("last_census".into(), last_census);
    payload.insert("uncovered_extensions".into(), Value::Object(uncovered));
    payload.insert("coverage".into(), Value::Object(counts.payload.clone()));
    payload.insert("complete".into(), json!(complete));
    payload.insert("incomplete_because".into(), json!(reasons));

    if counts.rechecked == 0 {
        payload.insert("recheck_note".into(), json!(
            "No judgement has been looked at a second time, so nothing here says \
             whether the judging was any good. Coverage proves every site was \
             looked at once. Run `recheck <sweep>` for an independent second look."
        ));
    }
    if counts.single_method {
        payload.insert("method_note".into(), json!(
            "Every judgement in this sweep was reached the same way, or with no \
             method recorded. Repeating a method reproduces its blind spots \
             exactly, so a second look by a different method is stronger evidence \
             than the same one applied again."
        ));
    }
    if !counts.declared_list {
        payload.insert("surfaces_note".into(), json!(
            "No parts of the system were declared for this audit, so this claim \
             covers the scope the census was given and not the system. Run \
             `surfaces <sweep> --propose` to see what is here."
        ));
    }
    println!("{}", output::json(&Value::Object(payload)));
}
