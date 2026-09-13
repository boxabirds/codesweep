//! The parts of the system this audit intends to examine.
//!
//! A surface is a named part of the system, not necessarily a directory. The
//! audit this exists for enumerated, among others, the secrets actually set in
//! each deployed environment and the contents of continuous integration,
//! neither of which has a path in the repository, and one of which held the
//! largest finding of the exercise. So the name is the identity and a scope is
//! an optional convenience the tool never acts on.
//!
//! This answers a different question from the uncovered-extension warning.
//! That one asks whether the rules reached everything inside the scope. This
//! asks whether the scope reached everything in the system.

use std::path::Path;

use serde_json::{Map, Value, json};

use crate::discovery;
use crate::model;
use crate::output;

/// What in a repository suggests a part of the system worth examining.
/// Deliberately short and biased towards precision: a proposal with twenty
/// entries gets accepted wholesale without being read, which is worse than
/// five that get edited.
const PACKAGE_MANIFESTS: &[&str] = &[
    "package.json", "Cargo.toml", "pyproject.toml", "setup.py", "go.mod",
    "pom.xml", "build.gradle", "build.gradle.kts", "Gemfile", "composer.json",
];

const CI_MARKERS: &[&str] = &[
    ".github/workflows/", ".gitlab-ci.yml", ".circleci/", "azure-pipelines.yml",
    "Jenkinsfile", ".buildkite/",
];

const HOOK_MARKERS: &[&str] = &[
    ".husky/", ".githooks/", "lefthook.yml", ".pre-commit-config.yaml",
];

const DOC_DIRS: &[&str] = &["docs/", "doc/", "documentation/"];

const DEPLOY_MARKERS: &[&str] = &[
    "wrangler.toml", "wrangler.jsonc", "Dockerfile", "docker-compose.yml",
    "docker-compose.yaml", "fly.toml", "vercel.json", "netlify.toml",
    "serverless.yml", "k8s/", "terraform/", ".env.example",
];

pub struct Candidate {
    pub name: String,
    pub scope: Option<String>,
    pub evidence: String,
}

/// Candidate surfaces, each with the evidence that suggested it.
///
/// Uses the same file discovery the census uses, so a candidate can never be a
/// part of the system the census would refuse to look at.
///
/// Incomplete by construction, and says so where it is printed. Two of the
/// surfaces in the audit that motivated this feature, the secrets actually set
/// in each deployed environment and the production data, leave no trace in a
/// repository. No scan will ever propose them, and the surface added latest in
/// that audit held its largest finding.
pub fn propose(root: &Path) -> (Vec<Candidate>, &'static str) {
    let found = discovery::discover(root);
    let mut seen: Vec<Candidate> = Vec::new();
    let mut add = |name: String, scope: Option<String>, evidence: &str, seen: &mut Vec<Candidate>| {
        if !seen.iter().any(|c| c.name == name) {
            seen.push(Candidate { name, scope, evidence: evidence.to_string() });
        }
    };

    for rel in &found.files {
        let parts: Vec<&str> = rel.split('/').collect();
        let base = parts[parts.len() - 1];

        if PACKAGE_MANIFESTS.contains(&base) && parts.len() > 1 {
            let pkg = parts[..parts.len() - 1].join("/");
            add(format!("package: {pkg}"), Some(pkg.clone()), rel, &mut seen);
        }
        for marker in CI_MARKERS {
            if rel == marker || rel.starts_with(marker) {
                add("automated checks".into(), None, rel, &mut seen);
            }
        }
        for marker in HOOK_MARKERS {
            if rel == marker || rel.starts_with(marker) {
                add("commit hooks".into(), None, rel, &mut seen);
            }
        }
        for d in DOC_DIRS {
            if rel.starts_with(d) {
                add("documentation".into(), Some(d.trim_end_matches('/').to_string()), rel, &mut seen);
            }
        }
        for marker in DEPLOY_MARKERS {
            if rel == marker || rel.starts_with(marker) {
                add("deployed configuration".into(), None, rel, &mut seen);
            }
        }
    }

    // Any top-level directory holding source the census could reach earns a
    // surface, unless a package manifest already claimed it or something inside
    // it. Derived from what is present rather than from a list of conventional
    // directory names, because a fixed list is a template by another name and
    // misses whatever this particular project happens to call things.
    let claimed: Vec<String> = seen.iter().filter_map(|c| c.scope.clone()).collect();
    for rel in &found.files {
        let parts: Vec<&str> = rel.split('/').collect();
        if parts.len() < 2 {
            continue;
        }
        let ext = Path::new(parts[parts.len() - 1])
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        if !crate::languages::SOURCE_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }
        let top = parts[0];
        if top.starts_with('.') {
            continue;
        }
        if claimed.iter().any(|c| c == top || c.starts_with(&format!("{top}/"))) {
            continue;
        }
        add(format!("source: {top}"), Some(top.to_string()), rel, &mut seen);
    }

    seen.sort_by(|a, b| a.name.cmp(&b.name));
    (seen, found.method.as_str())
}

pub struct Args<'a> {
    pub name: &'a str,
    pub propose: bool,
    pub add: Option<&'a str>,
    pub scope: Option<&'a str>,
    pub remove: Option<&'a str>,
    pub examined: Option<&'a str>,
    pub unexamined: Option<&'a str>,
}

pub fn run(root: &Path, args: Args<'_>) {
    let conn = super::open(root, false);
    super::require_sweep(&conn, args.name);

    if args.propose {
        let (candidates, method) = propose(root);
        let declared: Vec<String> = model::surfaces_of(&conn, args.name)
            .into_iter()
            .map(|s| s.name)
            .collect();
        let mut payload = Map::new();
        payload.insert("sweep".into(), json!(args.name));
        payload.insert("file_discovery".into(), json!(method));
        payload.insert(
            "proposed".into(),
            Value::Array(
                candidates
                    .into_iter()
                    .map(|c| {
                        let mut m = Map::new();
                        m.insert("name".into(), json!(c.name));
                        m.insert("scope".into(), json!(c.scope));
                        m.insert("evidence".into(), json!(c.evidence));
                        m.insert("already_declared".into(), json!(declared.contains(&c.name)));
                        Value::Object(m)
                    })
                    .collect(),
            ),
        );
        payload.insert("note".into(), json!(
            "A starting point, not a complete list. This is drawn only from \
             what leaves a trace in the repository. Parts of a system that \
             leave none, such as the secrets actually set in a deployed \
             environment or the contents of production data, will never be \
             proposed here and have to be named by hand. Add what is missing \
             and remove what does not apply."
        ));
        println!("{}", output::json(&Value::Object(payload)));
        return;
    }

    let known = |conn: &rusqlite::Connection| -> Vec<String> {
        model::surfaces_of(conn, args.name).into_iter().map(|s| s.name).collect()
    };

    if let Some(name) = args.add {
        if name.trim().is_empty() {
            super::die("a surface needs a name");
        }
        if known(&conn).iter().any(|n| n == name) {
            // Refused rather than merged: two surfaces sharing a name make the
            // coverage count mean nothing.
            super::die(&format!("surface '{name}' is already declared"));
        }
        let after = model::latest_census_id(&conn, args.name);
        conn.execute(
            "INSERT INTO surface (sweep, name, scope, examined_at, declared_at, \
             declared_after_census) VALUES (?1,?2,?3,NULL,?4,?5)",
            rusqlite::params![args.name, name, args.scope, output::now(), after],
        )
        .expect("the surface is declared");
    }

    for (flag, action) in [
        (args.remove, "remove"),
        (args.examined, "examined"),
        (args.unexamined, "unexamined"),
    ] {
        let Some(flag) = flag else { continue };
        if !known(&conn).iter().any(|n| n == flag) {
            super::die(&format!("no surface named '{flag}' in sweep '{}'", args.name));
        }
        match action {
            "remove" => {
                conn.execute(
                    "DELETE FROM surface WHERE sweep = ?1 AND name = ?2",
                    [args.name, flag],
                )
                .expect("the surface is removed");
            }
            "examined" => {
                conn.execute(
                    "UPDATE surface SET examined_at = ?1 WHERE sweep = ?2 AND name = ?3",
                    rusqlite::params![output::now(), args.name, flag],
                )
                .expect("the surface is marked");
            }
            _ => {
                conn.execute(
                    "UPDATE surface SET examined_at = NULL WHERE sweep = ?1 AND name = ?2",
                    [args.name, flag],
                )
                .expect("the surface is unmarked");
            }
        }
    }

    let rows = model::surfaces_of(&conn, args.name);
    let mut payload = Map::new();
    payload.insert("sweep".into(), json!(args.name));
    payload.insert(
        "surfaces".into(),
        Value::Array(
            rows.iter()
                .map(|r| {
                    let mut m = Map::new();
                    m.insert("name".into(), json!(r.name));
                    m.insert("scope".into(), json!(r.scope));
                    m.insert("examined".into(), json!(r.examined_at.is_some()));
                    m.insert(
                        "added_after_census".into(),
                        if r.declared_after_census == 0 {
                            Value::Null
                        } else {
                            json!(r.declared_after_census)
                        },
                    );
                    Value::Object(m)
                })
                .collect(),
        ),
    );
    for (key, value) in model::surface_counts(&conn, args.name) {
        payload.insert(key, value);
    }
    if rows.is_empty() {
        payload.insert("note".into(), json!(
            "No surfaces are declared. Coverage will be reported against the \
             scope given to the census, not against the system. Declare the \
             parts of the system this audit intends to examine and the claim \
             becomes relative to that list instead."
        ));
    }
    println!("{}", output::json(&Value::Object(payload)));
}
