//! Enumerate every candidate site. Deterministic, countable, and the agent
//! does not choose the set.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Map, Value, json};

use crate::discovery;
use crate::languages;
use crate::ledger::{self, STATE_GONE, STATE_LIVE};
use crate::model;
use crate::output;
use crate::rules::Rule;

pub struct Args<'a> {
    pub name: &'a str,
    pub rules: &'a [String],
    pub scope: Option<&'a str>,
    pub question: Option<&'a str>,
}

/// Source extensions present in the scope, counted.
fn scope_extensions(scope: &Path) -> (BTreeMap<String, usize>, discovery::Method) {
    let found = discovery::discover(scope);
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for name in &found.files {
        let ext = Path::new(name)
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        if languages::SOURCE_EXTENSIONS.contains(&ext.as_str()) {
            *counts.entry(ext).or_insert(0) += 1;
        }
    }
    (counts, found.method)
}

/// Source extensions in the scope that no census rule's language covers.
fn uncovered_extensions(
    present: &BTreeMap<String, usize>,
    rule_languages: &[String],
) -> (Map<String, Value>, Vec<String>) {
    let mut covered: Vec<&str> = Vec::new();
    let mut unknown: Vec<String> = Vec::new();
    for language in rule_languages {
        match languages::extensions_for(language) {
            Some(exts) => covered.extend(exts.iter().copied()),
            None => unknown.push(language.clone()),
        }
    }
    let mut uncovered = Map::new();
    for (ext, count) in present {
        if !covered.contains(&ext.as_str()) {
            uncovered.insert(ext.clone(), json!(count));
        }
    }
    unknown.sort();
    unknown.dedup();
    (uncovered, unknown)
}

pub fn run(root: &Path, args: Args<'_>) {
    ledger::sweep_stale_sessions();

    let scope = discovery::resolve_scope(root, args.scope);
    let scope = super::absolute(&scope.to_string_lossy());
    if !scope.exists() {
        super::die(&format!(
            "scope {} does not exist (relative scopes resolve against --root, {})",
            scope.display(),
            root.display()
        ));
    }
    for rule in args.rules {
        if !Path::new(rule).exists() {
            super::die(&format!("rule file {rule} does not exist"));
        }
    }

    let conn = super::open(root, true);
    let stamp = output::now();
    let scope_rel = super::relative_to(&scope, root);

    let existing: Option<String> = conn
        .query_row(
            "SELECT question FROM sweep WHERE name = ?1",
            [args.name],
            |r| r.get(0),
        )
        .ok();
    match &existing {
        None => {
            conn.execute(
                "INSERT INTO sweep (name, question, scope, created_at, updated_at) \
                 VALUES (?1,?2,?3,?4,?5)",
                rusqlite::params![
                    args.name,
                    args.question.unwrap_or_default(),
                    scope_rel,
                    stamp,
                    stamp
                ],
            )
            .expect("the sweep is created");
        }
        Some(recorded) => {
            // Changing the question changes what the verdicts mean, so a
            // rerun that supplies a different one is refused with the recorded
            // question quoted back rather than silently overwritten.
            if let Some(asked) = args.question {
                if asked != recorded {
                    super::die(&format!(
                        "sweep '{}' already asks: '{recorded}'\n\
                         Changing the question changes what the verdicts mean. Start a new sweep.",
                        args.name
                    ));
                }
            }
        }
    }

    // The census is only ever complete with respect to these rules.
    conn.execute("DELETE FROM rule WHERE sweep = ?1", [args.name])
        .expect("prior rules cleared");
    let mut loaded: Vec<(String, Rule)> = Vec::new();
    for path in args.rules {
        let rule = match Rule::load(Path::new(path)) {
            Ok(r) => r,
            Err(e) => super::die(&e.to_string()),
        };
        conn.execute(
            "INSERT INTO rule (sweep, rule_id, language, path, source, source_hash) \
             VALUES (?1,?2,?3,?4,?5,?6)",
            rusqlite::params![
                args.name,
                rule.id,
                rule.language,
                super::relative_to(&super::absolute(path), root),
                rule.source,
                ledger::truncated_sha256(rule.source.as_bytes(), ledger::SITE_ID_HEX_CHARS),
            ],
        )
        .expect("the rule is recorded");
        loaded.push((path.clone(), rule));
    }

    // Enumerate. One walk of the scope, every rule applied to each file whose
    // extension that rule's language covers, so a file is read once rather
    // than once per rule.
    let walked = discovery::discover(&scope);
    let mut seen: Vec<(String, SiteRow)> = Vec::new();
    let mut order: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut ordinals: std::collections::HashMap<(String, String, String), usize> =
        std::collections::HashMap::new();

    for (_path, rule) in &loaded {
        let exts = languages::extensions_for(&rule.language).unwrap_or(&[]);
        for relname in &walked.files {
            let ext = Path::new(relname)
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();
            if !exts.contains(&ext.as_str()) {
                continue;
            }
            let full = scope.join(relname);
            let Ok(bytes) = std::fs::read(&full) else { continue };
            let source = String::from_utf8_lossy(&bytes);
            let relfile = super::relative_to(&full, root);
            for m in rule.matches(&source) {
                let key = (rule.id.clone(), relfile.clone(), ledger::normalise(&m.text));
                let ordinal = *ordinals.get(&key).unwrap_or(&0);
                ordinals.insert(key, ordinal + 1);
                let site_id = ledger::site_identity(&rule.id, &relfile, ordinal, &m.text);
                let row = SiteRow {
                    rule_id: rule.id.clone(),
                    file: relfile.clone(),
                    start_line: m.start_line as i64,
                    end_line: m.end_line as i64,
                    ordinal: ordinal as i64,
                };
                match order.get(&site_id) {
                    Some(at) => seen[*at] = (site_id, row),
                    None => {
                        order.insert(site_id.clone(), seen.len());
                        seen.push((site_id, row));
                    }
                }
            }
        }
    }

    let mut prior_live: Vec<String> = conn
        .prepare("SELECT site_id FROM site WHERE sweep = ?1 AND state = ?2")
        .and_then(|mut s| {
            s.query_map([args.name, STATE_LIVE], |r| r.get::<_, String>(0))?
                .collect()
        })
        .unwrap_or_default();
    prior_live.sort();

    let mut added = 0i64;
    for (site_id, s) in &seen {
        let known: Option<String> = conn
            .query_row(
                "SELECT site_id FROM site WHERE sweep = ?1 AND site_id = ?2",
                [args.name, site_id],
                |r| r.get(0),
            )
            .ok();
        if known.is_none() {
            added += 1;
            conn.execute(
                "INSERT INTO site (sweep, site_id, rule_id, file, start_line, end_line, \
                 ordinal, state, first_seen, last_seen) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                rusqlite::params![
                    args.name, site_id, s.rule_id, s.file, s.start_line, s.end_line,
                    s.ordinal, STATE_LIVE, stamp, stamp
                ],
            )
            .expect("the site is recorded");
        } else {
            conn.execute(
                "UPDATE site SET start_line = ?1, end_line = ?2, state = ?3, last_seen = ?4 \
                 WHERE sweep = ?5 AND site_id = ?6",
                rusqlite::params![s.start_line, s.end_line, STATE_LIVE, stamp, args.name, site_id],
            )
            .expect("the site is refreshed");
        }
    }

    let found_ids: Vec<&String> = seen.iter().map(|(id, _)| id).collect();
    let departed: Vec<&String> = prior_live
        .iter()
        .filter(|id| !found_ids.contains(id))
        .collect();
    for site_id in &departed {
        conn.execute(
            "UPDATE site SET state = ?1 WHERE sweep = ?2 AND site_id = ?3",
            rusqlite::params![STATE_GONE, args.name, site_id],
        )
        .expect("the departed site is marked");
    }

    let (present, discovery_method) = scope_extensions(&scope);
    let rule_languages: Vec<String> = loaded.iter().map(|(_, r)| r.language.clone()).collect();
    let (uncovered, unknown) = uncovered_extensions(&present, &rule_languages);

    conn.execute(
        "INSERT INTO census_run (sweep, ran_at, found, added, departed, uncovered) \
         VALUES (?1,?2,?3,?4,?5,?6)",
        rusqlite::params![
            args.name,
            stamp,
            seen.len() as i64,
            added,
            departed.len() as i64,
            output::json_compact(&Value::Object(uncovered.clone()))
        ],
    )
    .expect("the census run is recorded");
    conn.execute(
        "UPDATE sweep SET updated_at = ?1 WHERE name = ?2",
        [&stamp, &args.name.to_string()],
    )
    .expect("the sweep is touched");

    let counts = model::coverage(&conn, args.name);
    let mut sorted_languages = rule_languages.clone();
    sorted_languages.sort();
    sorted_languages.dedup();

    let mut payload = Map::new();
    payload.insert("sweep".into(), json!(args.name));
    payload.insert("scope".into(), json!(scope_rel));
    payload.insert(
        "rules".into(),
        json!(loaded.iter().map(|(_, r)| r.id.clone()).collect::<Vec<_>>()),
    );
    payload.insert("rule_languages".into(), json!(sorted_languages));
    payload.insert("sites_found".into(), json!(seen.len()));
    payload.insert("sites_new".into(), json!(added));
    payload.insert("sites_departed".into(), json!(departed.len()));
    payload.insert("unjudged".into(), json!(counts.unjudged));
    payload.insert("file_discovery".into(), json!(discovery_method.as_str()));
    payload.insert("note".into(), json!(
        "The census is complete with respect to these rules only. A site \
         whose code changed since it was judged returns as new and needs \
         judging again."
    ));
    if !uncovered.is_empty() {
        payload.insert("WARNING_uncovered_extensions".into(), Value::Object(uncovered));
        payload.insert("WARNING".into(), json!(
            "The scope contains source files with extensions that NO census rule \
             covers, so those files were never examined and cannot appear in the \
             report. ast-grep treats tsx as a language separate from typescript, \
             and jsx separately from javascript, so a .tsx file needs its own rule \
             with `language: tsx`. Add a rule per language or narrow the scope. \
             Coverage arithmetic below counts only the files the rules can reach."
        ));
    }
    if !unknown.is_empty() {
        payload.insert("WARNING_unknown_rule_languages".into(), json!(unknown));
        if !payload.contains_key("WARNING") {
            payload.insert("WARNING".into(), json!(
                "One or more rule languages are absent from resweep's extension map, \
                 so the uncovered-extension check could not run for them. Verify by hand \
                 that every file you expect to be swept was reached."
            ));
        }
    }
    println!("{}", output::json(&Value::Object(payload)));
}

struct SiteRow {
    rule_id: String,
    file: String,
    start_line: i64,
    end_line: i64,
    ordinal: i64,
}
