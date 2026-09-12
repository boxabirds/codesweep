---
name: codesweep
description: Run a whole-codebase audit that is provably complete rather than repeated until it goes quiet. Use whenever the task is to find EVERY place in a codebase matching some condition - every hardcoded colour, every swallowed error, every unguarded call, every violation of a project rule - or to plan a refactor that must touch every affected site. Also use when a previous audit's findings need refreshing after code changed. Do NOT use for locating one known thing, for reading a single file, or for open-ended exploration.
---

# codesweep

## Why this exists

An agent asked to audit a codebase picks which files to read. The candidate set
is therefore unknown, and so is what was missed. Running the audit again and
again until nothing new turns up is a stopping heuristic, not coverage. It stops
when the model runs out of ideas, which has no relationship to whether the
codebase ran out of violations.

This is measured, not folklore. On multi-file tasks the dominant published
failure mode is incomplete refactoring: agents change some affected sites and
never reach the rest.

codesweep removes the choice. `ast-grep` enumerates every candidate site from a
declared rule. You judge the sites it hands you, one at a time. Coverage becomes
subtraction.

**You never decide what the candidate set is. You only judge what is in it.**

## Two ceilings, stated before anything else

**Enumeration is syntactic, never semantic.** ast-grep does no scope, type or
dataflow analysis. Given a top-level `save()`, a shadowed local `save()` inside
another function, and `obj.save()`, the pattern `save($A)` matches the first two
identically and misses the third. So a sweep is complete with respect to a
**syntactic shape**, never with respect to a **symbol**. If the question is
genuinely "every call site of this function", codesweep is the wrong tool and a
language server's find-references is the right one.

**Complete enumeration does not buy correct judgement.** A well-tuned LLM judge
run over a provably complete finding set reached 96% agreement with human triage
on true positives and only 41% on false positives. Fixing enumeration gets you a
correct denominator. It does not get you correct answers. Expect a second
failure mode after the first is gone, and treat a `pass` verdict as weaker
evidence than a `violation` one.

## The protocol

Follow these steps in order. Do not skip steps 2, 3 or 5.

### 1. State the question

Write the audit question as a sentence a person could answer about one site.
"Which of these catch clauses swallows an error the caller needed to see?" is
answerable per site. "Is our error handling any good?" is not.

If the question cannot be answered about a single site in isolation, codesweep is
the wrong tool. Say so rather than forcing it.

### 2. Write the census rule

The census rule defines the candidate set, so **the rule is the only thing
standing between you and a false completeness claim**. A rule that under-matches
produces a small, clean, confident, wrong report.

Write the rule to describe **candidacy, not violation**. It selects what gets
looked at. It does not decide anything.

```yaml
id: ts-catch-clause
language: typescript
rule:
  kind: catch_clause
```

That is the right shape: broad, dumb, and incapable of hiding a violation. Resist
the urge to encode the verdict into the rule, like excluding any catch that
contains a `throw`. That looks efficient and it silently drops every catch that
rethrows a *different* error while swallowing the original. Narrow the rule only
for a reason you can state, and when you do, say in the report exactly what the
narrowing excluded, because those sites will never appear.

Broad rule plus more judgements costs tokens. Narrow rule costs correctness, and
costs it invisibly.

**One rule file per language.** ast-grep treats `tsx` as a language separate from
`typescript`, and `jsx` separately from `javascript`. A rule saying
`language: typescript` silently skips every `.tsx` file. This is not theoretical:
it is how the rule originally shipped with this skill would have swept a React
codebase, enumerated only the `.ts` files, printed `unjudged: 0`, and reported
zero violations while every real violation sat in `.tsx`. Write one rule per
language and pass them all to a single census.

Keep rule files somewhere durable and version-controlled, next to the code they
audit. Do not leave them in a temp directory. The report reproduces the rule
source in full, but the path it records should still be one a reader can open.

### 3. Prove the rule before censusing

Three checks, all three, every time:

- **Census positive.** Name a site you already believe belongs in the candidate
  set. Confirm the rule matches it. Matching is not a verdict, only membership.
- **Census negative.** Name a site that should not be a candidate at all.
  Confirm the rule does not match it.
- **Independent count.** Count the candidates a second way, with `rg` or `grep`,
  and compare. This is the check that catches a missing language: a zero is
  obviously broken, but 22 when the truth is 56 looks perfectly plausible and
  will not catch your eye. Only a second, independent count catches it.

Use `ast-grep scan -r rule.yml --json=compact <path>` to see raw matches, and
`--debug-query=ast` to see how a pattern parsed when it will not match.

Report what you checked, with numbers. "Rule matched 56, ripgrep counted 56
across .ts and .tsx" is evidence. "The rule looks right" is not.

### 4. Census

```bash
codesweep --root /abs/path/to/repo census <sweep-name> \
  --rule rules/ts-catch-clause.yml \
  --rule rules/tsx-catch-clause.yml \
  --scope src \
  --question "..."
```

Always pass `--root`. It names the repository being swept, and a relative
`--scope` resolves against it rather than against the working directory.

**Read the census output before judging anything.** It reports the site count,
and it warns under `WARNING_uncovered_extensions` when the scope contains source
files that no rule's language can reach. That warning means those files were
never examined and cannot appear in the report. Add a rule per language or
narrow the scope. Never judge a single site while that warning stands.

Report the site count to the operator. If it is far larger than expected, refine
the rule now, not after spending the judgements.

The index lives in a per-session temporary directory, never in the repository.
It is rebuilt by every census, so it cannot go stale and there is nothing to
refresh. It disappears with the session. Nothing you run leaves state behind in
the operator's repo.

### 5. Take the manifest, before judging anything

```bash
codesweep --root /abs/path/to/repo manifest <sweep-name>
```

This lists every candidate as one line, grouped under its file. Do this
immediately after the census and before any judging, because it is what makes
the sweep citable: the identifiers enter the conversation as a fixed set you can
refer back to for the rest of the session, rather than a number you have to
remember.

A few dozen sites costs a few kilobytes. Use `--file <path>` to read a large
sweep in parts, and `--unjudged` to see what is left. The index holds the
complete record either way, so a partial listing loses nothing.

If the conversation is compacted and the manifest scrolls out of context, run it
again. The index is still there and the site ids are unchanged.

### 6. Drain the queue

```bash
codesweep --root /abs/path/to/repo next <sweep-name> --limit 15
```

This returns unjudged sites with surrounding source. Judge each one, then record
the whole batch:

```bash
codesweep --root /abs/path/to/repo verdict <sweep-name> --from-json - <<'JSON'
[
  {"site_id": "a1b2c3d4e5f6a7b8", "verdict": "violation",
   "note": "Catches the D1 error and returns an empty array, so a caller cannot distinguish an empty table from a failed query."},
  {"site_id": "b2c3d4e5f6a7b8c9", "verdict": "pass",
   "note": "Rethrows after logging; the caller still sees the failure."}
]
JSON
```

Verdicts are `violation`, `pass` or `na`. Every verdict needs a note saying what
you saw at that site. An empty note is refused, because a verdict without a
reason is not a judgement.

Use `na` only when the rule matched something the question does not apply to, for
example a catch clause inside a test fixture. `na` is not an escape from a hard
site. If a site is hard, read the file.

**Judge the site, not the chain.** A site that is correct in isolation but part
of a broken chain, such as a handler that returns a proper error which its caller
then discards, is a `pass` at that site. Record the chain in the note and
name the other site. Marking both ends `violation` double-counts one defect;
marking the correct end `na` hides it. The note is where the chain lives.

To revisit a site after it has left the queue:

```bash
codesweep --root /abs/path/to/repo show <sweep-name> --site <site_id>
```

Loop `next` then `verdict` until `next` returns no sites. This is the part that
takes real work. Do not stop early and do not summarise from the sites you
happened to see.

### 7. Report

```bash
codesweep --root /abs/path/to/repo status <sweep-name>
codesweep --root /abs/path/to/repo report <sweep-name> -o <path>.md
```

`status` prints `live_sites`, `judged` and `unjudged`. The report states the
coverage figures and reproduces each census rule in full, so a reader can see
exactly what the completeness claim covers without needing the rule files.

Follow the project's own convention for where reports live, if it has one.

## What you may and may not claim

You may say: **"N sites enumerated, N judged, M violations, complete with respect
to rules X and Y."**

You may not say "audit complete", "no violations found", "the codebase is clean",
or "I checked everywhere" without a `status` output showing `unjudged: 0` **and**
a census with no uncovered-extension warning. Run `status` to check rather than
counting from memory; that is what it is for. If either fails, the finding is
partial and you must say so with the number.

The completeness claim is always relative to the census rules and to syntax.
State this. It is the honest limit of the technique, and hiding it turns a real
guarantee into a false one.

## Re-auditing after code changes

Run `census` again with the same sweep name. Sites whose code is unchanged keep
their verdicts. Sites whose code changed come back as new and unjudged. Sites
that disappeared are marked gone and drop out of the coverage arithmetic.

A site's identity is its rule, its file and its code with whitespace collapsed.
Reformatting does not invalidate a verdict. A real edit does, deliberately,
because the verdict was about code that no longer exists.

So a re-audit after a small change costs a handful of judgements rather than the
whole sweep. The output names `sites_new` and `sites_departed` so you can see
what moved.

Verdicts last as long as the session and no longer. There is nothing to commit
and nothing to clean up. If the work needs to outlive the session, write the
report to a file, which is the artefact a person can read anyway.

## Driving a refactor

The same ledger runs a refactor rather than an audit. Census the sites to change.
Judge each one `violation` when it needs the change and `pass` when it does not.
Then work the violation list from `report`, and re-run `census` afterwards: a
site you have correctly changed comes back as a **new** site, which is the signal
that the edit landed and now needs re-judging as `pass`.

Do not use `ast-grep --rewrite` for a mechanical fix across the violation list
unless the operator has approved a bulk rewrite. Enumerate, judge, then change.

When the refactor is finished, the completeness assertion is "zero occurrences of
the old pattern remain". Offer to add the census rule to the project's lint or CI
configuration so that claim becomes a gate rather than a memory.

## Failure modes to avoid

- **Ignoring the uncovered-extension warning.** It means part of the scope was
  never looked at. Everything downstream of it is a false claim.
- **Judging from the excerpt alone.** `next` gives surrounding lines, not the
  whole file. An excerpt cannot tell you whether an error state is ever rendered.
  Read the file when it cannot settle the question, and expect to do so often.
- **Batch-judging by pattern.** Ten sites that look alike may not be alike. The
  point of the ledger is per-site judgement.
- **Marking `na` to clear the queue.** This converts a coverage guarantee back
  into a guess while keeping the appearance of rigour.
- **Reporting the census count as the violation count.** The census counts
  candidates. Only verdicts count violations.
- **Trusting a rule that returned zero.** Debug it. Zero is almost always a
  broken rule.
- **Trusting a rule that returned a plausible number.** Plausible is what a
  missing language looks like. Only the independent count catches it.
- **Judging without taking the manifest first.** Without it the candidate set is
  a number you are holding in your head, which is the habit this replaces.

## Requirements

`ast-grep` on PATH (`brew install ast-grep`). codesweep refuses to run without
it rather than falling back to a text search, because a text search cannot give
the guarantee this whole tool exists to provide.
