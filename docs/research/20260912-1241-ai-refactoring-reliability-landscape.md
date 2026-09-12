# Tools for reliable AI refactoring and whole-codebase audits

Researched 12 September 2026. Sources were fetched during the research session.
Anything not confirmed by fetching a page is marked UNVERIFIED and should be
re-checked before it is relied on.

The question driving this: what gives a coding agent reliable whole-codebase
structural awareness, so that an audit or refactor does not need repeated lossy
passes? The distinction that matters throughout is whether a tool gives
**deterministic enumeration** of candidate sites, so a sweep is provably
complete, or whether it is approximate.

## The short answer

Tooling that gives provable enumeration exists and is mature, but it is all
either compiler and build integrated (SCIP, Kythe, Glean, LSP) or syntactic only
(ast-grep, Comby, Zoekt). Everything marketed *to agents* between 2025 and 2026,
meaning repo maps, embeddings and tree-sitter knowledge graphs, is approximate by
construction and does not admit a completeness proof.

Every industrial success at scale splits the problem the same way: deterministic
tooling finds the sites, and the model only judges or transforms each site.
Nobody sells that split as a product for a general TypeScript codebase.

## Structural search and rewrite

| Tool | Status | Enumeration |
|---|---|---|
| ast-grep | Healthy, v0.45.3 as of 31 Aug 2026. The `sg` binary was deprecated in 0.45.0. | Deterministic, syntactic only |
| Comby | Effectively dormant. Last release June 2022, commits resumed June 2026. | Deterministic, syntactic |
| Semgrep | Healthy, LGPL-2.1. | Deterministic, including dataflow and taint |
| GritQL | Ownership changed. Honeycomb acquired Grit in 2025 and the repos were donated to Biome, where it is now a plugin language. | Deterministic, syntactic |
| OpenRewrite | Very healthy. | Deterministic and semantic, via its own lossless syntax tree |
| jscodeshift | Alive. | Deterministic, syntactic |
| Codemod | Active, Apache-2.0. Wraps ast-grep in TypeScript, ships an MCP server. | Deterministic engine |
| Sourcegraph Batch Changes | Enterprise only. Pricing starts at sixteen thousand dollars with a minimum annual contract. Self-serve is gone. | See below |

### The ast-grep ceiling

From ast-grep's own FAQ, verbatim: it does not support "scope analysis, type
information, control flow analysis, data flow analysis, taint analysis, constant
propagation", and explicitly cannot "distinguish between variable definitions and
references".

This was verified locally rather than taken on trust. Given a file containing a
top-level `save()`, a shadowed local `save()` inside another function, and
`obj.save()`, the pattern `save($A)` matched the first two identically and missed
the third.

So ast-grep gives a provably complete list of syntactic matches, and its
`--json` output is exactly the deterministic worklist you want. It cannot tell
you all call sites of a *symbol*. That is a real ceiling, not a nitpick.

The official ast-grep MCP server is search only. It exposes tree dumping, rule
testing and two find tools. There is no rewrite tool. A third-party server that
does dry-run replacement has no meaningful adoption and should not be depended
on.

### Sourcegraph Agentic Batch Changes

The closest commercial fit to the enumerate-then-judge architecture. Public beta
on Cloud 30 June 2026, self-hosted in 7.5 on 8 July 2026. It uses Sourcegraph
search to identify repositories needing the change, generates deterministic
scripts for mechanical changes, and delegates judgement changes to Claude Code or
Codex through its MCP.

Caveats: the price floor above, a multi-repository orientation, and organisational
risk from Sourcegraph splitting, with Amp spinning out as a separate company.

### OpenRewrite's limit

Open source support covers Java, Kotlin and Groovy only. JavaScript, TypeScript,
C#, Python, Ruby, Go and COBOL are Moderne proprietary, and `rewrite-python` is
archived. For a TypeScript codebase, OpenRewrite is not available.

Moderne's local MCP server is the best deterministic-tools-for-agents product
that exists, exposing structural search, type and method finders, roughly seven
thousand recipes, and targeted refactorings. It is JVM only. Gartner named
Moderne a Leader in its 2026 quadrant for AI-augmented code modernisation, and
the companion report found rule-based deterministic tools outperform pure LLM
refactoring.

### Language servers, now reachable by agents

- **JetBrains** ships a built-in MCP server since IntelliJ IDEA 2025.2, bundled
  and enabled by default, exposing rename refactoring, call analysis and symbol
  search.
- **Serena** is the strongest maintained free option, with roughly twenty-nine
  thousand stars, MIT licensed, abstracting LSP over more than forty languages.
  Its find-referencing-symbols is real LSP find-references, so enumeration is
  sound. The critical caveat: it fails **silently incomplete** when the project
  does not resolve, for example when dependencies are not installed or the
  TypeScript config is wrong, and nothing warns you.
- **mcp-language-server** is popular but roughly six months stale.

### Google's prior art

ClangMR, from a 2013 paper, applied Clang ASTs across the monorepo with MapReduce.
Rosie splits a large change into independently reviewable and submittable shards.
Both are internal and unavailable.

## Indexing and code graphs

**Deterministic tier, build integrated.** SCIP is active and is where enumeration
is provable, with indexers for TypeScript, Java, Rust, Python and others, and
every occurrence carrying a definition or reference role. LSIF is dead, removed
from Sourcegraph in 4.6. Kythe is more alive than its reputation and is what
Google's migration pipeline actually uses, though it still requires
compiler-wrapping extractors. Glean has active commits but zero releases and
zero tags, so it is build-from-source only, and its query language is documented
as limited to non-recursive queries.

**Cold.** GitHub archived stack-graphs in September 2025 with a README saying it
is no longer supported or updated. GitHub's current code navigation docs no
longer mention precise navigation or stack graphs at all. Whether GitHub still
runs it internally is UNVERIFIED. tree-sitter-graph, its DSL layer, has not seen
a commit since December 2024.

**Exact but textual.** tree-sitter itself is very healthy but parses per file
with no cross-file name binding. Zoekt returns every matching line and has no
idea which `foo` is which. universal-ctags warns in its own man page that macros
can fool it into missing tags, and its reference tags are name-matched rather
than resolved.

**Approximate tier, which is what agents actually use.** Aider's repo map was
confirmed from source: tree-sitter tags into a graph, then PageRank, then
truncation to a token budget. It is a relevance heuristic that silently omits,
and Aider itself has been quiet since May 2026. Cursor uses a Merkle tree for
cache invalidation and embeddings for retrieval. Cline deliberately does not
index, arguing that chunking code for embeddings tears apart its logic and that
an index is a snapshot frozen in time. Roo Code uses tree-sitter blocks into a
vector store with a cosine threshold.

Two large 2026 projects are worth knowing about. CodeGraph pairs a Rust kernel
and tree-sitter with SQLite, does cross-file reference resolution, and unusually
**publishes its own incompleteness**, reporting resolved-dependent coverage per
language, from roughly 96% for TypeScript down to roughly 85% for C#, and stating
plainly that the residual is a genuine static-analysis frontier. Note that this
is file-level coverage, a much weaker guarantee than call-site level. GitNexus is
similar in ambition but is PolyForm Noncommercial licensed, which disqualifies it
for commercial use.

One widely-shared comparison page labels Aider's repo map and CodeGraph
"deterministic". That column means "not embeddings", not "provably complete". Its
star counts are also stale. Do not cite it.

## Agent-specific audit reliability

This is the thinnest category, and it is the one closest to the problem.

**agentcov**, from Trail of Bits, Apache-2.0, describes itself as "gcov for what
lines of code agents read". It captures reads through Codex hooks, backfills from
Claude Code and Codex transcripts, and parses shell reads including `sed`, `grep`,
`rg`, `cat`, `head` and `tail`. It emits LCOV, HTML heatmaps and an unread-lines
report, and records unsupported read shapes as unknown rather than guessing.
It is the only implementation found of the obvious idea of tracking which files
an agent actually looked at. It has roughly fifty stars.

**cloudflare/security-audit-skill**, MIT, is the coverage-ledger pattern
productised. It runs reconnaissance into a `coverage-ledger.json`, assigns
isolated hunters from ledger units, uses "coverage critics" to find unmapped
surface, has fresh verifiers attempt to disprove each finding, keeps validators
separate from discoverers, and validates the ledger after every update. It is
security-shaped but the skeleton transfers.

**Semgrep Assistant** is the pattern running in production with published
numbers. Deterministic rules enumerate findings and an LLM judges each finding
with rule metadata, prior triage decisions and dataflow context. Over a triaged
set of more than two thousand findings it moved from roughly 55% agreement to
96% on true positives, but reached only **41% agreement on false positives**.
That number is the one to remember: a well-tuned per-item judge over a provably
complete enumeration is still unreliable in one direction. Note also that
Semgrep's MCP server is archived, despite third-party pages still describing it
as maintained.

## The evidence that recall failure is real

| Source | Metric | Result |
|---|---|---|
| Multi-Agent Coordinated Rename (arXiv 2601.00482) | Precision and recall over identifiers needing coordinated change | Vanilla LLM precision 31%, recall 17%. Averages five compilation errors per rename set |
| Beyond Accuracy: Multi-Hunk Repair (arXiv 2511.11012) | Localisation success, covering all ground-truth buggy files | Codex 75.25%, Claude Code 66.09%, Gemini CLI 49.26% |
| SWE-Bench ProMax (arXiv 2608.09802) | Resolve rate on multi-file tasks | Best 41.2%. Authors name incomplete refactoring as the dominant failure mode |
| SWE Refactor Bench (arXiv 2608.23564) | Passing migration, behavioural and verification gates | 28 of 520 runs, 5.4%. Thirteen of twenty tasks had no accepted solution from any model |

Tests are a weak oracle for this. Differential fuzzing of LLM refactorings
(arXiv 2602.15761) found 19 to 35% are functionally non-equivalent, with about
21% of those undetected by the existing test suites. A separate study found 29.6%
of test-passing SWE-bench patches behave differently from ground truth.

FreshBrew is the sharpest illustration of the trade-off: OpenRewrite scored 7.0%
overall on a Java 8 to 17 migration but 78.0% on the subset where its syntax tree
build succeeded. Deterministic tools are brittle in a different way from models.
The same study documented reward hacking, with one model excluding failing tests
from the build file and another wrapping a failing call so tests silently skip.

## What industry actually does

**Google.** From their own paper on internal migrations, verbatim: the parts that
identify locations to change and validate that the right thing took place are
handled "mostly using deterministic AST techniques", which have the advantage of
being "always correct" and not suffering from model version changes. On one
migration, 80% of landed modifications were AI-authored and roughly half the time
was saved. Discovery is Kythe, not a model. Critically, they walk references only
to a bounded distance and state plainly that beyond it they can miss. Google,
with a monorepo and Kythe, does not publish a recall figure and admits it can
miss.

**Airbnb** migrated roughly three and a half thousand test files in six weeks
against a manual estimate of eighteen months, reaching 75% in the first four
hours and 97% after four days of tuning, with long-tail files retried fifty to a
hundred times each. Important framing caveat: that file set was enumerable by
grep, so this is evidence about per-site transformation reliability, not about
recall.

**Meta** filters hard. Their test-generation work built tests for 75% of targets,
of which 25% increased coverage, and only verifiably-improving output reaches a
human. Their hardening work turned roughly eleven thousand classes into nine
thousand mutants and five hundred and seventy-one tests.

**Amazon** Q Code Transformation's public blog reports an 85% higher success rate
than their own previous approach, never defines success, and gives no absolute
pass rate. The widely-repeated figures about thirty thousand applications and
developer-years saved are UNVERIFIED.

## Gaps nobody fills

1. **Nothing bridges semantic enumeration to agent-driven per-site judgement
   outside the JVM.** For a TypeScript codebase there is no product that hands an
   agent every reference and returns a completion receipt.
2. **No end-to-end recall figure exists anywhere.** Not in industry, not in
   academia. Anyone claiming their agent "found everything" has no basis for it.
3. **No sweep-completion receipt format.** No standard artifact saying the
   candidate set was N, each was judged, here are the M changed and the rest
   skipped with reasons. Cloudflare's coverage ledger is the nearest thing and is
   security-specific.
4. **Coverage tracking of agent reads is a fifty-star project.** That nobody
   larger built it suggests the industry has not yet accepted that incomplete
   sweeps are the failure mode.
5. **Precise cross-file navigation for agents is either paid or archived.**
   The free path is Serena over LSP, which fails silently incomplete.
6. **Enumeration completeness is not judgement correctness, and nobody prices
   that in.** Semgrep's 41% is the honest ceiling.
7. **No tool inverts the sweep automatically.** After a migration the completeness
   assertion is that zero occurrences of the old pattern remain, expressible as a
   lint rule in CI. Every codemod tool can express it. None generates it as a
   by-product of the transformation.

## What this implies for codesweep

Gaps 3 and 1 are what codesweep occupies, which is why building rather than
adopting was the right call. Gaps 5 and 6 are the ceilings it inherits and
cannot remove, and both are stated in its skill and in every report it writes.

Worth adopting rather than rebuilding:

- **agentcov** as a complement, to catch the case where an agent never opened a
  file at all, which is a different failure from never enumerating it.
- **Serena** when the question genuinely needs symbol-level references rather
  than syntactic shapes, with the silent-incompleteness caveat made explicit.
- **The inverse assertion**, turning a finished sweep's census rule into a CI
  lint rule, which converts completeness from a claim into a gate. Cheapest
  correct thing available and it needs no new tooling.

## Items not to repeat without re-checking

Whether GitHub still runs stack-graphs internally; Sourcebot's symbol-resolution
backend; the date Sourcegraph withdrew self-serve pricing; Amazon's application
and developer-year claims; all Uber, Grab and Stripe numbers.
