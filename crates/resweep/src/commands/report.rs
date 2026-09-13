//! Markdown findings with an explicit coverage claim.

use std::path::Path;

use serde_json::{Map, Value};

use crate::ledger::{STATE_LIVE, VERDICTS};
use crate::model;
use crate::output;

pub fn run(root: &Path, name: &str, destination: Option<&str>) {
    let conn = super::open(root, false);
    let (question, scope) = super::require_sweep(&conn, name);
    let counts = model::coverage(&conn, name);

    let mut stmt = conn
        .prepare("SELECT rule_id, language, path, source FROM rule WHERE sweep = ?1 ORDER BY rule_id")
        .expect("the rule query compiles");
    let rules: Vec<(String, String, String, String)> = stmt
        .query_map([name], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .expect("rules read")
        .filter_map(Result::ok)
        .collect();

    let uncovered: Map<String, Value> = conn
        .query_row(
            "SELECT uncovered FROM census_run WHERE sweep = ?1 ORDER BY id DESC LIMIT 1",
            [name],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();

    let mut stmt = conn
        .prepare(
            "SELECT s.file, s.start_line, s.end_line, s.rule_id, s.site_id, v.note \
             FROM site s JOIN verdict v ON v.sweep = s.sweep AND v.site_id = s.site_id \
             WHERE s.sweep = ?1 AND s.state = ?2 AND v.verdict = 'violation' \
             ORDER BY s.file, s.start_line",
        )
        .expect("the violation query compiles");
    let violations: Vec<(String, i64, i64, String, String, String)> = stmt
        .query_map([name, STATE_LIVE], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
        })
        .expect("violations read")
        .filter_map(Result::ok)
        .collect();

    let by_verdict = counts.payload.get("by_verdict").cloned().unwrap_or(Value::Null);
    let count_of = |v: &str| -> i64 {
        by_verdict.get(v).and_then(|n| n.as_i64()).unwrap_or(0)
    };

    let mut out: Vec<String> = Vec::new();
    out.push(format!("# Sweep: {name}"));
    out.push(String::new());
    out.push(format!("**Question.** {question}"));
    out.push(String::new());
    out.push(format!("**Scope.** `{scope}`"));
    out.push(String::new());
    out.push("## Coverage".into());
    out.push(String::new());
    out.push("| | |".into());
    out.push("|---|---|".into());
    out.push(format!("| Candidate sites enumerated | {} |", counts.live_sites));
    out.push(format!("| Sites judged | {} |", counts.judged));
    out.push(format!("| Sites unjudged | {} |", counts.unjudged));
    for verdict in VERDICTS {
        out.push(format!("| Verdict `{verdict}` | {} |", count_of(verdict)));
    }
    out.push(String::new());

    if !uncovered.is_empty() {
        let listed: Vec<String> = uncovered
            .iter()
            .map(|(ext, n)| format!("{n} `{ext}` files"))
            .collect();
        out.push(format!(
            "> **This sweep did not look at part of its own scope.** It contains \
             {} that no census rule's language can reach, so nothing in those \
             files was ever a candidate and no violation in them can appear \
             below. Add a rule per language and re-census before treating these \
             findings as a picture of the scope.",
            listed.join(", ")
        ));
        out.push(String::new());
    }

    let (complete, reasons) = model::completeness(&counts, &uncovered);
    if !complete {
        out.push("> This sweep is INCOMPLETE, because:".into());
        out.push(">".into());
        for reason in &reasons {
            out.push(format!("> - {reason}"));
        }
        out.push(">".into());
        out.push("> Findings below are partial.".into());
    } else {
        out.push(
            "> Every enumerated site carries a verdict, so this sweep is complete \
             with respect to the census rules below and nothing else."
                .into(),
        );
    }
    if !counts.declared_list {
        out.push(">".into());
        out.push(
            "> No parts of the system were declared for this audit, so the claim \
             above covers the scope the census was given rather than the system."
                .into(),
        );
    }
    out.push(String::new());

    let surfaces = model::surfaces_of(&conn, name);
    if !surfaces.is_empty() {
        out.push("## Parts of the system this audit set out to examine".into());
        out.push(String::new());
        out.push(
            "Completeness above is relative to this list as well as to the rules. \
             Every part is shown, not only the outstanding ones, because the list \
             is the scope of the claim."
                .into(),
        );
        out.push(String::new());
        out.push("| Part | Scope | Examined | Added after the list was written |".into());
        out.push("|---|---|---|---|".into());
        let baseline = surfaces.iter().map(|s| s.declared_after_census).min().unwrap_or(0);
        for s in &surfaces {
            let late = if s.declared_after_census > baseline { "yes" } else { "no" };
            out.push(format!(
                "| {} | {} | {} | {late} |",
                s.name,
                s.scope.clone().unwrap_or_else(|| "no path in the repository".into()),
                if s.examined_at.is_some() { "yes" } else { "no" }
            ));
        }
        out.push(String::new());
        if surfaces.iter().any(|s| s.declared_after_census > baseline) {
            out.push(
                "This list grew during the audit. A part nobody thought of at the \
                 start is evidence the list may still be short, and a part added \
                 late is not less likely to matter than one thought of first."
                    .into(),
            );
            out.push(String::new());
        }
    }

    out.push("## Census rules".into());
    out.push(String::new());
    out.push(
        "Completeness is relative to these rules. A violation shaped differently \
         from every rule here was never a candidate and does not appear. The rule \
         source is reproduced in full so this claim stays checkable after the rule \
         files themselves have moved or gone."
            .into(),
    );
    out.push(String::new());
    for (rule_id, language, path, source) in &rules {
        out.push(format!("### `{rule_id}` ({language})"));
        out.push(String::new());
        out.push(format!("From `{path}` at census time."));
        out.push(String::new());
        out.push("```yaml".into());
        out.push(source.trim_end().to_string());
        out.push("```".into());
        out.push(String::new());
    }
    out.push(
        "Matching is syntactic. ast-grep does no scope, type or dataflow analysis, \
         so it cannot tell one `foo` from another `foo` in a different scope, and a \
         call written in a different shape is a different pattern. A sweep is \
         complete with respect to a syntactic shape, never with respect to a symbol."
            .into(),
    );
    out.push(String::new());
    out.push(format!("## Violations ({})", violations.len()));
    out.push(String::new());
    if violations.is_empty() {
        out.push("None recorded.".into());
    }
    for (file, start, end, rule_id, site_id, note) in &violations {
        out.push(format!("### `{file}:{start}`"));
        out.push(String::new());
        out.push(format!("Rule `{rule_id}`, site `{site_id}`, lines {start}-{end}."));
        out.push(String::new());
        out.push(output::fill(note, 88));
        out.push(String::new());
    }

    let text = out.join("\n");
    match destination {
        Some(path) => {
            if let Err(e) = std::fs::write(path, format!("{text}\n")) {
                super::die(&e.to_string());
            }
            println!("wrote {path}");
        }
        None => println!("{text}"),
    }
}
