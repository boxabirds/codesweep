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

## What this is for

Three buckets, because the difference between them decides whether this tool
helps you or wastes your time.

### Badly served today, or not served at all

**Knowing when you are finished.** Everything else here exists for this. The
normal way to sweep a codebase is to search, fix what you find, search again
with a different pattern, and stop when nothing new turns up. That stopping
point is a statement about running out of ideas, not about the codebase running
out of instances. codesweep fixes the candidate set before any judging starts,
so what is left is subtraction.

**A candidate set you can cite.** After a sweep there is a list of every site,
each with a stable id, and a report that reproduces the rules that produced it.
Six months later you can re-run the same rule and diff. "We checked" becomes a
number and a manifest.

**Picking up a half-finished audit.** Verdicts are recorded per site as you go,
so a sweep interrupted by anything at all resumes where it stopped. A site whose
code changed since you judged it comes back as new; one whose formatting changed
does not.

**Noticing a whole file type was never looked at.** The failure this product was
built after: a rule declaring one language silently skips every file of its
sibling language, the queue drains, and the report is clean because half the
codebase was never opened. The census now reports every source extension in
scope that no rule can reach, and refuses to call the sweep complete while any
remain.

### Improved

**Finding every instance of a syntactic shape.** Text search does this badly and
the failure is quiet in both directions. Measured on a file containing three
catch clauses: a careful regular expression, the kind a person actually writes,
found one of the three. A naive one found all three plus a comment and a string.
AST matching found exactly three.

**Auditing against a project convention.** Every hardcoded colour, every
swallowed error, every use of a deprecated thing. The census gives you the
denominator and the ledger keeps your judgements.

**Checking that a past change landed everywhere.** Same rule, run again, diff
the manifest.

How much better than a capable agent doing the same job by hand is not yet
measured. A suite that runs each question with and without the tool and reports
the difference is being built; until it has run, treat the improvement here as
argued rather than demonstrated.

### Out of scope, by design

**Anything that depends on which declaration a name refers to.** Renaming a
method, finding every caller, working out what breaks if you delete a class.
Matching is syntactic. Given a top-level `save()`, a shadowed local `save()` and
`obj.save()`, a pattern matches the first two identically and misses the third.
This is not a gap to be closed with a better rule; it needs a compiler or a
language server, and you should use one.

**One literal with one spelling.** If the answer is exact from a single search,
take the single search. Enumeration machinery buys you nothing here and makes it
less credible when you genuinely need it.

**Whether the findings are right.** A correct denominator is not correct
answers. Semgrep ran a tuned LLM judge over a provably complete finding set and
reached 96% agreement with human triage on true positives and 41% on false
positives. Expect a second failure mode after this one is gone, and treat a
`pass` as weaker evidence than a `violation`.

**Reading a file, or exploring.** Not what this is.

## Install

```sh
git clone https://github.com/boxabirds/codesweep.git
cd codesweep
./install.sh
```

It links the CLI onto your PATH and the skill into `~/.claude/skills`, then
checks that both actually work rather than assuming the links took. Re-running
is safe: it reports what is already in place and changes only what is not. It
will not overwrite anything that is not its own symlink.

Needs `ast-grep` and Python 3.9 or newer. The installer checks for both and
tells you how to get `ast-grep` if it is missing, rather than half-installing.

```sh
./install.sh --check      # report state, change nothing
./install.sh --uninstall  # remove the links it created
```

The skill installs at user level, so it is available in every project. Claude
Code picks it up without a restart, but sessions already open will not see it
until they restart.

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

## User journey

A real run, start to finish, against axios 1.13.2 at `8092aee`. Every number and
quote below came from executing it, not from writing what it would look like.

### 1. Install

`./install.sh`, as above. Once, not per project.

### 2. Ask a question in Claude Code

> does axios swallow any errors in a way callers cannot see?

You do not name the skill. The description is in the skill listing, the model
recognises the shape of the question, and invokes it.

### 3. The agent writes a rule, and proves it

```yaml
id: js-catch-clause
language: javascript
rule:
  kind: catch_clause
```

Deliberately dumb: it selects candidates, it decides nothing. Then the
cross-check the skill insists on. Ripgrep counted 19 catch lines in `lib/`,
codesweep found 18.

That gap is not waved through. The extra line was
`asyncExecutor(...).catch(_reject)`, a promise method that matches the regex and
is not a catch clause. The AST count was right and the text search over-counted.
A disagreement in the other direction would have meant a broken rule.

### 4. Census

```
sites: 18 | uncovered: none
```

0.24 seconds. `uncovered: none` means every source file in scope was reachable
by a rule. When it is not, the census says so loudly and `status` refuses to
report complete. That guard exists because the rule originally shipped with this
skill declared `language: typescript`, which silently skips every `.tsx` file: on
a React codebase it enumerated 22 sites, reported `unjudged: 0`, and found
nothing, while all the real violations sat in the files it never opened.

### 5. Manifest

```
sweep: swallowed
question: Does this catch clause hide a failure from the caller?
rules: js-catch-clause
listed 18 of 18 live sites, 18 unjudged

lib/adapters/adapters.js
  7df0d8bd7bc1ff56  29-31  -
lib/adapters/fetch.js
  e1950380ee0be71c  27-29  -
  9235a9942ed29d2d  245-258  -
```

1,092 bytes for the whole set. This is the candidate list entering the
conversation as something the agent can cite for the rest of the session, rather
than a count it has to remember.

### 6. Judging, which is the part that costs real work

`next` hands out batches with surrounding source, matched lines marked `>`. The
excerpt is a starting point, not the evidence. One site here could not be settled
from eight lines of context, so the agent read `lib/core/Axios.js` and then
`lib/core/InterceptorManager.js` to answer it. The skill tells it to do exactly
that.

Every verdict needs a note. An empty one is refused, because a verdict without a
reason is not a judgement.

### 7. Status gates the claim

```json
{ "live_sites": 18, "judged": 18, "unjudged": 0,
  "by_verdict": { "pass": 16, "violation": 2 }, "complete": true }
```

The skill forbids saying an audit is done while this says otherwise. A claim the
agent would otherwise make from memory becomes one it has to check.

### The outcome

Two violations in eighteen, and the second is a real bug in axios.

**`lib/core/Axios.js:178`** calls `onRejected.call(this, error)` in the
synchronous interceptor path. `InterceptorManager.use(fulfilled)` called with a
single argument stores `rejected: undefined`, which is how most people register
an interceptor. So a request interceptor that throws, registered without a
rejection handler, raises `TypeError: Cannot read properties of undefined
(reading 'call')` and destroys the original error. Both halves verified in the
source.

**`lib/adapters/http.js:359`** catches a throw from `abortEmitter.emit`, writes
`console.warn`, and continues. Remaining abort listeners never run, and the
caller of `abort()` sees success.

### What it actually cost

Census, manifest and status together took **0.43 seconds**. That is the
"whole codebase, in seconds" part, and it holds.

The judging did not take seconds. It took eighteen judgements and two file reads.
What that buys is a number you can check: every catch clause in `lib/` was
examined, none was skipped, and the two that matter are named with reasons.

An ordinary agent pass reads four or five files, finds the conspicuous empty
catch in `adapters.js`, and stops. That one is a false positive: it wraps setting
a function's `name` property, it carries an eslint exemption, and it has no
caller-visible consequence. Both real violations are in files that pass would
never have opened.

Sixteen passes is also a result. The tool did not manufacture findings.

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

## The guard, working against its own author

`examples/20260912-0826-css-literal-colour-audit.md` is a sweep of a real web app
for hardcoded colours. 37 sites enumerated, 37 judged, 29 violations, and 8
compliant uses of `rgba(var(--token-rgb), alpha)` that the broad rule surfaced
and per-site judgement cleared.

Two findings a file-by-file read would plausibly have missed: `--accent-primary`
is referenced in two component files and defined nowhere in the palette, so those
declarations resolve to nothing; and one status badge is the only one whose
colours are bare hex with no dark mode override.

The part worth reading it for is the header. That sweep used a CSS rule only, so
it never opened the 118 `.tsx` and 59 `.ts` files in the same directory, which
hold a further 148 lines carrying literal colours. The report says so at the top
and `status` reports `complete: false`. An earlier version of this tool called
that audit complete.

## Tests

```sh
tests/run-all.sh
```

**`tests/test_install.sh`**, 27 assertions against temporary target directories,
so it never touches the real `~/.claude` or PATH. A first install creating both
links, a re-run changing nothing, a link pointing elsewhere being repointed and
saying what it replaced, a real file where a link belongs being refused rather
than overwritten, `--check` changing nothing, `--uninstall` removing its own
links while leaving a foreign file alone, and installing not modifying any
tracked file.

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
