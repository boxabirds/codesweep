<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/icon-dark.svg">
  <img src="assets/icon.svg" alt="codesweep" width="120" align="right">
</picture>

# codesweep

**Auditing and refactoring safely means knowing every place a change has to
land. Coding agents don't know.** They read some files and tell you what they
found. They can't tell you what they missed, and neither can you.

codesweep makes the candidate set countable. `ast-grep` enumerates every site
matching a rule, the agent judges them one at a time, and coverage becomes
arithmetic instead of a claim you have to take on trust.

That is not complete knowledge of your codebase. It is complete enumeration of
one shape at a time, which is the part agents were getting wrong.

| | |
|---|---|
| **Enumeration** | `ast-grep` lists every candidate site from a declared rule. Deterministic, countable, repeatable. |
| **Judgement** | The agent judges one site at a time and every verdict is recorded against a stable site id. |
| **Coverage** | Subtraction. `live_sites - judged = unjudged`. Not a feeling. |

The agent never decides what the candidate set is. It only judges what is in it.

## Install

Requires `ast-grep` on PATH and Python 3.9+. No Python dependencies.

```sh
brew install ast-grep
ln -s "$PWD/bin/codesweep" /usr/local/bin/codesweep   # or anywhere on PATH
ln -s "$PWD/skills/codesweep" ~/.claude/skills/codesweep
```

The second link is what matters: an agent reaches this through the skill, and
the skill drives the CLI. Installing the directory as a plugin with
`claude --plugin-dir .` works too. A skill installed at user level is available
in every project, and Claude Code picks it up without a restart, though sessions
already running will not see it until they are restarted.

## Using it as a skill

This is mostly not something you type. It is a skill an agent invokes when it
realises it needs to look for something that could be anywhere, and the agent
then drives the CLI itself.

You do not name it. Claude Code injects the skill's description into a listing
and the model decides, so the description is the trigger. Those below were
measured, not guessed: a fresh agent was given the description alone and asked,
for each prompt, whether it would invoke the skill first. The wording was
revised over five rounds against the results.

**Fires.** The shared property is that the answer is a set whose size has to be
right, not a location.

- find every place we hardcode a colour instead of using a palette token
- audit every catch clause in workers/taskmgr for silent fallbacks
- which agent-facing strings are hardcoded instead of going through getPrompt()
- are we still using the deprecated toast component anywhere
- we need to remove all silent fallbacks before launch, where are they
- migrate every rgba(255,255,255,x) to a palette token

Note the third and fourth carry no "every" or "all". A plural question about
rule violations wants the whole set, and the description says so explicitly,
because without that clause it fired only by luck.

**Does not fire, correctly.**

- read the deploy script and tell me what it does — one file
- what does the taskmgr worker do — open-ended, no condition to match
- how many times do we call console.log in workers/admin — one literal string,
  one pattern, so a plain `grep -c` answers it exactly
- rename getPrompt to loadPrompt everywhere — a refactor touching every site,
  but it needs symbol resolution
- find all callers of createProject — same
- find everything that would break if I delete the LimitExceededError class —
  same

The last three are the important exclusions. They look exactly like sweeps and
they are the cases ast-grep cannot serve, because it does no scope or type
analysis and cannot tell one `save` from another `save` in a different scope.
That needs a language server. Before the description said so, renames fired.

You can still force it with `/codesweep`, or skip the skill and drive the CLI
yourself.

## What the agent sees

A real sweep of the Ceetrix admin worker, abbreviated only where marked.

**Census** enumerates. The count is the denominator, and nothing about it is the
agent's to choose.

```json
{
  "sweep": "swallowed",
  "rules": ["ts-catch-clause", "tsx-catch-clause"],
  "rule_languages": ["tsx", "typescript"],
  "sites_found": 56,
  "sites_new": 56,
  "unjudged": 56,
  "file_discovery": "git",
  "WARNING_uncovered_extensions": { ".css": 1, ".html": 1 },
  "WARNING": "The scope contains source files with extensions that NO census rule covers, so those files were never examined and cannot appear in the report. ..."
}
```

That warning is the guard against the failure this tool exists to prevent. Here
it is benign, naming a stylesheet and an HTML file a catch-clause rule was never
going to match. Run the same census with only the `typescript` rule and it names
27 `.tsx` files instead, which is how a sweep returns 22 sites, reports
`unjudged: 0`, and finds nothing while every real violation sits in the files it
never opened.

**Manifest** is the citable record, and the step that makes the sweep referable
later in the conversation. One line per site, grouped under its file.

```
sweep: swallowed
question: Does this catch clause swallow a failure the caller needed to see?
rules: ts-catch-clause, tsx-catch-clause
listed 56 of 56 live sites, 55 unjudged

workers/admin/src/frontend/components/CreatePlanModal.tsx
  3549d1f2cfe0228d  151-153  pass
workers/admin/src/frontend/components/DiffModal.tsx
  c89323d99c256c1e  99-102  -
  2c8e026cdc52812a  118-122  -
```

Roughly 56 bytes per site, so a sweep this size costs about 3KB to hold in
context for the rest of the session. The trailing column is the verdict, or `-`
where none has been given.

If the conversation is compacted and the listing scrolls out of context, run it
again. The index outlives the context window and the site ids do not change.

**Next** hands out a batch with surrounding source, the matched lines marked
with `>`.

```
>   151 |     } catch (err) {
>   152 |       setErrors({ submit: err instanceof Error ? err.message : 'Failed to create plan' });
>   153 |     } finally {
    154 |       setIsSubmitting(false);
```

The context is a starting point, not the evidence. Eight lines cannot tell you
whether an error state is ever rendered, so a judging agent reads the file when
the excerpt does not settle it. An independent evaluation of this skill found
the agent opening a dozen files beyond the excerpts across 56 sites.

**Verdict** records one judgement per site. An empty note is refused, because a
verdict without a reason is not a judgement.

**Status** is the arithmetic, and it is what gates the completeness claim.

```json
{
  "coverage": { "live_sites": 56, "judged": 1, "unjudged": 55, "by_verdict": { "pass": 1 } },
  "uncovered_extensions": { ".css": 1, ".html": 1 },
  "complete": false
}
```

`complete` is false here for two independent reasons: sites remain unjudged, and
part of the scope was unreachable by any rule. Either alone is enough. The skill
forbids claiming an audit is done while this says otherwise, which converts a
claim the agent would otherwise make from memory into one it has to check.

**Report** renders the findings as markdown, reproducing each census rule in
full so the completeness claim stays checkable after the rule files have moved.
See `examples/` for two real ones.

## Driving the CLI directly

```sh
codesweep census my-audit --rule rules/css-literal-colour.yml --scope src \
  --question "Is this literal colour a design-system violation?"
codesweep manifest my-audit               # every site, one line, the citable record
codesweep next my-audit --limit 15        # unjudged sites with source context
codesweep verdict my-audit --from-json -  # [{site_id, verdict, note}]
codesweep show my-audit --site <id>       # re-read one site after judging
codesweep status my-audit                 # coverage arithmetic
codesweep report my-audit -o audit.md
```

Pass `--root` on every call. It names the repository being swept, and a relative
`--scope` resolves against it rather than against the working directory.

Verdicts are `violation`, `pass` or `na`. A verdict without a note is refused,
because a verdict without a reason is not a judgement.

## Where the index lives, and for how long

In a per-session temporary directory keyed by `CLAUDE_CODE_SESSION_ID`, never in
the repository being swept. Set `CODESWEEP_SESSION_ID` to run outside a session.
With neither, codesweep refuses rather than falling back to a shared location,
because two unrelated runs sharing one index would silently merge their
candidate sets.

Nothing is left in your repository and nothing survives the session. A census
rebuilds the index in a fraction of a second, so there is nothing worth keeping
and nothing that can go stale. Abandoned session directories are removed by age
on a later run.

Two sessions cannot collide, because each has its own index. Were results ever
merged they would be additive rather than conflicting, since a site's identity is
content-addressed: the same code yields the same id whoever enumerated it.

## Re-auditing within a session is cheap

Re-run `census` with the same sweep name. A site whose code is unchanged keeps
its verdict. A site whose code changed comes back as new and unjudged. A site
that disappeared is marked gone and leaves the arithmetic.

A site's identity is its rule, its file and its code with whitespace collapsed,
so reformatting does not invalidate a verdict but a real edit does.

## Three limits, stated plainly

**Completeness is relative to the census rule.** A violation shaped differently
from every rule in the sweep was never a candidate and will not appear. Every
report says so and reproduces its rules in full.

**Matching is syntactic, never semantic.** ast-grep does no scope, type or
dataflow analysis. Given a top-level `save()`, a shadowed local `save()` in
another function and `obj.save()`, the pattern `save($A)` matches the first two
identically and misses the third. A sweep is complete with respect to a
syntactic shape, never with respect to a symbol. For real reference enumeration
you want a language server, not this.

**Complete enumeration does not buy correct judgement.** Semgrep ran a tuned LLM
judge over a provably complete finding set and reached 96% agreement with human
triage on true positives and 41% on false positives. Fixing enumeration gets you
a correct denominator, not correct answers.

So the rule is the one thing standing between you and a confident, clean, wrong
report. Guard it:

- Prove the rule against a census positive and a census negative, then **count
  the candidates a second way with `rg`**. A zero is obviously broken. A
  plausible-but-wrong number is what a missing language looks like, and only an
  independent count catches it.
- Write one rule per language. ast-grep treats `tsx` as separate from
  `typescript` and `jsx` from `javascript`. `census` now refuses to let this pass
  silently: it reports every source extension in the scope that no rule can
  reach, `status` reports `complete: false` while any remain, and `report` says
  in prose what it did not look at.
- Prefer a broad rule plus more judgements over a clever narrow rule. Breadth
  costs tokens. Narrowness costs correctness, silently.

## Worked example

`examples/20260912-0826-css-literal-colour-audit.md` is a real sweep of the
Ceetrix web app for hardcoded colours. One rule, 37 enumerated sites, 37 judged,
29 violations and 8 compliant uses of `rgba(var(--token-rgb), alpha)` that the
broad rule correctly surfaced and per-site judgement correctly cleared.

Two findings came out of it that a file-by-file read would plausibly have missed:
`--accent-primary` is referenced in two component files but defined nowhere in
the palette, and the QA status badge is the one badge whose colours are bare hex
with no dark mode override.

That example also demonstrates the guard working against its own author. The
sweep used a CSS rule only, so it never looked at the 118 `.tsx` and 59 `.ts`
files in the same directory, which hold a further 148 lines carrying literal
colours. The report says so at the top and `status` reports `complete: false`.
The first version of this tool would have called that audit complete.

## Tests

```sh
tests/run-all.sh
```

**`tests/test_codesweep.sh`**, 58 assertions against a fixture with a
hand-counted number of sites. Enumeration, rule narrowing, refused empty notes,
refused unknown site ids, coverage arithmetic, verdict survival across
reformatting, re-judging after a real edit, site departure on file deletion, the
uncovered-language guard firing on a `.tsx` file and clearing when a `tsx` rule
is added, relative scope resolution, the manifest matching the census count, a
run with no session being refused, a session id that could escape the temp root
being refused, and stale session directories being cleaned by age.

**`tests/test_against_ceetrix.sh`**, 18 assertions against a real monorepo. A
fixture proves the mechanism but cannot prove the tool survives mixed languages,
generated files, vendored bundles and a `.gitignore` that matters. None of these
assertions freeze a count, because the repository changes daily and a frozen
count would fail for the wrong reason and then get deleted. Each is either an
invariant that holds whatever the repository contains, such as the `typescript`
and `tsx` counts summing exactly to the combined count, or a cross-check against
ripgrep as an independent oracle. It skips cleanly when no checkout is present;
point `CEETRIX_REPO` at one to run it.
