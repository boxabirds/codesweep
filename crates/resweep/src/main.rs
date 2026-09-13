//! The command line.
//!
//! Ten commands, no more and no fewer. A rewrite that also adds a command
//! cannot be verified against the thing it replaces.
//!
//! Argument-parsing refusals are rendered to match the tool being replaced
//! rather than left to the parsing library's own wording. Those messages are
//! in the recorded reference and the guidance quotes them, so a better-worded
//! refusal is still a broken contract.

use clap::{Parser, Subcommand};

use resweep::commands;
use resweep::model::{DEFAULT_BATCH_SIZE, DEFAULT_CONTEXT_LINES};

/// The subcommands, in the order the parser offers them, used to render a
/// refusal that names the alternatives.
const COMMANDS: &[&str] = &[
    "census", "next", "verdict", "surfaces", "recheck", "status", "report",
    "manifest", "show", "list",
];

/// Argparse exits with two on a parsing failure and one on everything the tool
/// itself refuses. Scripts read the difference.
const EXIT_USAGE: i32 = 2;

const DESCRIPTION: &str = "\
resweep - deterministic enumeration + judgement ledger for whole-codebase audits.";

#[derive(Parser)]
#[command(name = "resweep", about = DESCRIPTION, disable_help_subcommand = true)]
struct Cli {
    /// repository root; may also be given after the subcommand
    #[arg(long = "root", global = true)]
    root: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// enumerate every candidate site
    Census {
        name: String,
        /// rule file (repeatable)
        #[arg(long = "rule", required = true)]
        rule: Vec<String>,
        /// subdirectory to sweep; defaults to root
        #[arg(long)]
        scope: Option<String>,
        /// the audit question, in plain words
        #[arg(long)]
        question: Option<String>,
    },
    /// hand out the next batch of unjudged sites
    Next {
        name: String,
        #[arg(long, default_value_t = DEFAULT_BATCH_SIZE)]
        limit: usize,
        #[arg(long, default_value_t = DEFAULT_CONTEXT_LINES)]
        context: usize,
    },
    /// record a judgement against a site
    Verdict {
        name: String,
        #[arg(long)]
        site: Option<String>,
        #[arg(long, value_parser = resweep::ledger::VERDICTS.to_vec())]
        verdict: Option<String>,
        #[arg(long, default_value = "")]
        note: String,
        /// how this judgement was reached, e.g. 'read the source'
        #[arg(long, default_value = "")]
        method: String,
        /// file of [{site_id,verdict,note,method}], or - for stdin
        #[arg(long = "from-json")]
        from_json: Option<String>,
        /// record an independent second judgement, leaving the first untouched
        #[arg(long = "second-opinion")]
        second_opinion: bool,
    },
    /// declare which parts of the system this audit intends to examine
    Surfaces {
        name: String,
        /// suggest surfaces from the repository
        #[arg(long)]
        propose: bool,
        /// declare a surface by name
        #[arg(long)]
        add: Option<String>,
        /// optional path for the surface being added
        #[arg(long)]
        scope: Option<String>,
        /// undeclare a surface by name
        #[arg(long)]
        remove: Option<String>,
        /// mark a surface as examined
        #[arg(long)]
        examined: Option<String>,
        /// mark a surface as not examined after all
        #[arg(long)]
        unexamined: Option<String>,
    },
    /// offer judged sites for an independent second look, first verdict hidden
    Recheck {
        name: String,
        #[arg(long, default_value_t = 5)]
        limit: usize,
        /// recheck one specific site
        #[arg(long)]
        site: Option<String>,
        #[arg(long, default_value_t = DEFAULT_CONTEXT_LINES)]
        context: usize,
    },
    /// coverage arithmetic for one sweep
    Status { name: String },
    /// markdown findings with an explicit coverage claim
    Report {
        name: String,
        #[arg(short = 'o', long)]
        output: Option<String>,
    },
    /// every site as one line, the citable record of the candidate set
    Manifest {
        name: String,
        /// restrict the listing to one file
        #[arg(long)]
        file: Option<String>,
        /// list only sites with no verdict
        #[arg(long)]
        unjudged: bool,
    },
    /// re-read one site, judged or not, with more context
    Show {
        name: String,
        #[arg(long, required = true)]
        site: String,
        #[arg(long, default_value_t = DEFAULT_CONTEXT_LINES * 3)]
        context: usize,
    },
    /// every sweep in this repository
    List,
}

fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => refuse(e),
    };
    // Absent, the root means the working directory, which is only ever right
    // when it is already the repository root.
    let root = commands::absolute(cli.root.as_deref().unwrap_or("."));

    match cli.command {
        Command::Census { name, rule, scope, question } => commands::census::run(
            &root,
            commands::census::Args {
                name: &name,
                rules: &rule,
                scope: scope.as_deref(),
                question: question.as_deref(),
            },
        ),
        Command::Next { name, limit, context } => commands::next::run(&root, &name, limit, context),
        Command::Verdict { name, site, verdict, note, method, from_json, second_opinion } => {
            commands::verdict::run(
                &root,
                commands::verdict::Args {
                    name: &name,
                    site: site.as_deref(),
                    verdict: verdict.as_deref(),
                    note: &note,
                    method: &method,
                    from_json: from_json.as_deref(),
                    second_opinion,
                },
            )
        }
        Command::Surfaces { name, propose, add, scope, remove, examined, unexamined } => {
            commands::surfaces::run(
                &root,
                commands::surfaces::Args {
                    name: &name,
                    propose,
                    add: add.as_deref(),
                    scope: scope.as_deref(),
                    remove: remove.as_deref(),
                    examined: examined.as_deref(),
                    unexamined: unexamined.as_deref(),
                },
            )
        }
        Command::Recheck { name, limit, site, context } => {
            commands::recheck::run(&root, &name, limit, site.as_deref(), context)
        }
        Command::Status { name } => commands::status::run(&root, &name),
        Command::Report { name, output } => commands::report::run(&root, &name, output.as_deref()),
        Command::Manifest { name, file, unjudged } => {
            commands::manifest::run(&root, &name, file.as_deref(), unjudged)
        }
        Command::Show { name, site, context } => commands::show::run(&root, &name, &site, context),
        Command::List => commands::list::run(&root),
    }
}

/// The usage blocks, written out rather than taken from the parsing library.
///
/// They are part of the recorded contract: the reference holds them and the
/// guidance quotes the refusals that follow them. A library's own rendering is
/// its own, and changes when the library does.
fn usage(subcommand: Option<&str>) -> String {
    let body = match subcommand {
        None => format!("[-h] [--root ROOT_GLOBAL] {{{}}} ...", COMMANDS.join(",")),
        Some("census") => "[-h] [--root ROOT] --rule RULE [--scope SCOPE] [--question QUESTION] name".into(),
        Some("next") => "[-h] [--root ROOT] [--limit LIMIT] [--context CONTEXT] name".into(),
        Some("verdict") => "[-h] [--root ROOT] [--site SITE] [--verdict {violation,pass,na}] \
[--note NOTE] [--method METHOD] [--from-json FROM_JSON] [--second-opinion] name"
            .into(),
        Some("surfaces") => "[-h] [--root ROOT] [--propose] [--add ADD] [--scope SCOPE] \
[--remove REMOVE] [--examined EXAMINED] [--unexamined UNEXAMINED] name"
            .into(),
        Some("recheck") => "[-h] [--root ROOT] [--limit LIMIT] [--site SITE] [--context CONTEXT] name".into(),
        Some("status") => "[-h] [--root ROOT] name".into(),
        Some("report") => "[-h] [--root ROOT] [-o OUTPUT] name".into(),
        Some("manifest") => "[-h] [--root ROOT] [--file FILE] [--unjudged] name".into(),
        Some("show") => "[-h] [--root ROOT] --site SITE [--context CONTEXT] name".into(),
        Some("list") => "[-h] [--root ROOT]".into(),
        Some(other) => format!("[-h] [--root ROOT] {other}"),
    };
    match subcommand {
        None => format!("usage: resweep {body}"),
        Some(name) => format!("usage: resweep {name} {body}"),
    }
}

fn refuse(error: clap::Error) -> ! {
    use clap::error::{ContextKind, ErrorKind};

    // Help and version are not refusals; they go to standard output and exit
    // cleanly, which is what the caller asked for.
    if matches!(
        error.kind(),
        ErrorKind::DisplayHelp
            | ErrorKind::DisplayVersion
            | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    ) {
        error.print().ok();
        std::process::exit(0);
    }

    let context = |kind: ContextKind| -> Option<String> {
        error.get(kind).map(|v| v.to_string())
    };
    // Which subcommand was being parsed, so the usage block and the error
    // prefix name it the way argparse does.
    let subcommand = std::env::args()
        .skip(1)
        .find(|a| COMMANDS.contains(&a.as_str()));

    let (prefix, message) = match error.kind() {
        ErrorKind::InvalidSubcommand | ErrorKind::UnknownArgument if subcommand.is_none() => {
            let given = context(ContextKind::InvalidSubcommand)
                .or_else(|| context(ContextKind::InvalidArg))
                .or_else(|| std::env::args().nth(1))
                .unwrap_or_default();
            (
                None,
                format!(
                    "argument command: invalid choice: '{given}' (choose from {})",
                    COMMANDS.join(", ")
                ),
            )
        }
        ErrorKind::MissingSubcommand => (
            None,
            "the following arguments are required: command".to_string(),
        ),
        ErrorKind::MissingRequiredArgument => {
            let what = context(ContextKind::InvalidArg).unwrap_or_else(|| "command".into());
            let names: Vec<String> = what
                .split('\n')
                .map(|line| argparse_name(line.trim()))
                .filter(|n| !n.is_empty())
                .collect();
            (
                subcommand.clone(),
                format!("the following arguments are required: {}", names.join(", ")),
            )
        }
        ErrorKind::InvalidValue => {
            let arg = context(ContextKind::InvalidArg).unwrap_or_default();
            let given = context(ContextKind::InvalidValue).unwrap_or_default();
            let allowed = resweep::ledger::VERDICTS.join(", ");
            (
                subcommand.clone(),
                format!(
                    "argument {}: invalid choice: '{given}' (choose from {allowed})",
                    argparse_name(&arg)
                ),
            )
        }
        _ => (subcommand.clone(), error.to_string().trim().to_string()),
    };

    eprintln!("{}", usage(prefix.as_deref()));
    match prefix {
        Some(name) => eprintln!("resweep {name}: error: {message}"),
        None => eprintln!("resweep: error: {message}"),
    }
    std::process::exit(EXIT_USAGE)
}

/// Turn the parsing library's rendering of an argument into argparse's.
/// `--verdict <VERDICT>` becomes `--verdict`, and a positional keeps its name.
fn argparse_name(arg: &str) -> String {
    arg.split_whitespace().next().unwrap_or(arg).to_string()
}
