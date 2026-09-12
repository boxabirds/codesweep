# codesweep

A coding agent asked to audit a codebase picks which files to read. So the
candidate set is unknown, and so is what it missed. Running the audit again until
nothing new turns up is a stopping heuristic, not coverage: it stops when the
model runs out of ideas, which has nothing to do with whether the codebase ran
out of violations.

codesweep takes the choice away.

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
ln -s "$PWD/bin/codesweep" /usr/local/bin/codesweep
```

For Claude Code, copy `skills/codesweep` into `~/.claude/skills/` or install
this directory as a plugin with `claude --plugin-dir .`.

## Use

```sh
codesweep census my-audit --rule rules/css-literal-colour.yml --scope src \
  --question "Is this literal colour a design-system violation?"
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

## The manifest is the citable record

`census` returns counts. `manifest` returns every candidate as one line grouped
under its file, which is what puts a stable identifier for each site into the
conversation. Roughly 56 bytes per site measured against a real sweep, so a few
dozen sites costs a few kilobytes.

Take it immediately after the census. If the conversation is compacted and the
listing scrolls away, run it again; the index is still there and the ids are
unchanged.

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
