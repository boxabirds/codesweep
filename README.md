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

Pass `--root` on every call; the ledger lives at `<root>/.codesweep/ledger.db`
and a relative `--scope` resolves against it, never against the working
directory.

Verdicts are `violation`, `pass` or `na`. A verdict without a note is refused,
because a verdict without a reason is not a judgement.

## Re-auditing is cheap

Re-run `census` with the same sweep name. A site whose code is unchanged keeps
its verdict. A site whose code changed comes back as new and unjudged. A site
that disappeared is marked gone and leaves the arithmetic.

A site's identity is its rule, its file and its code with whitespace collapsed,
so reformatting does not invalidate a verdict but a real edit does. That is what
makes a standing audit affordable: after a small change you re-judge a handful of
sites, not the whole codebase.

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
tests/test_codesweep.sh
```

36 assertions over a fixture with a hand-counted number of sites, covering
enumeration, rule narrowing, refused empty notes, refused unknown site ids,
coverage arithmetic, verdict survival across reformatting, re-judging after a
real edit, site departure on file deletion, the uncovered-language guard firing
on a `.tsx` file and clearing when a `tsx` rule is added, relative scope
resolving against `--root` rather than the working directory, and the coverage
claim in the report.
