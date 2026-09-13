//! Every sweep in this repository.

use serde_json::{Map, Value, json};

use crate::model;
use crate::output;

pub fn run(root: &std::path::Path) {
    let conn = super::open(root, false);
    let mut stmt = conn
        .prepare("SELECT name, question, scope, updated_at FROM sweep ORDER BY updated_at DESC")
        .expect("the sweep table exists");
    let rows: Vec<(String, String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .expect("sweeps read")
        .filter_map(Result::ok)
        .collect();
    let payload: Vec<Value> = rows
        .into_iter()
        .map(|(name, question, scope, updated_at)| {
            let mut m = Map::new();
            m.insert("sweep".into(), json!(name));
            m.insert("question".into(), json!(question));
            m.insert("scope".into(), json!(scope));
            m.insert("updated_at".into(), json!(updated_at));
            m.insert("coverage".into(), Value::Object(model::coverage(&conn, &name).payload));
            Value::Object(m)
        })
        .collect();
    println!("{}", output::json(&Value::Array(payload)));
}
