# Sweep: admin-swallowed-errors

**Question.** Does this catch clause swallow a failure, so that the caller cannot tell a failure apart from a legitimate empty or default result?

**Scope.** `workers/admin/src`

## Coverage

| | |
|---|---|
| Candidate sites enumerated | 56 |
| Sites judged | 56 |
| Sites unjudged | 0 |
| Verdict `violation` | 6 |
| Verdict `pass` | 50 |
| Verdict `na` | 0 |

> Every enumerated site carries a verdict, so this sweep is complete with respect to the census rules below and nothing else.

## Census rules

Completeness is relative to these rules. A violation shaped differently from every rule here was never a candidate and does not appear.

- `ts-catch-clause-ts` (typescript) from `../../../../private/tmp/claude-501/-Users-julian-expts-claude-backlog/223cb291-8894-4a61-a4fa-f022289c80de/scratchpad/rules/catch-ts.yml`
- `ts-catch-clause-tsx` (tsx) from `../../../../private/tmp/claude-501/-Users-julian-expts-claude-backlog/223cb291-8894-4a61-a4fa-f022289c80de/scratchpad/rules/catch-tsx.yml`

## Violations (6)

### `workers/admin/src/frontend/components/DiffModal.tsx:99`

Rule `ts-catch-clause-tsx`, site `ecb49b7ba48c5f1b`, lines 99-102.

DiffModal.tsx:99 - parseDiff failure returns [] to the files useMemo. The render at line
210 says `files.length === 0 || files.every(...)` -> 'No differences found', which is
exactly the legitimate empty-diff message. console.error reaches the devtools console,
not the user. A parse failure is therefore indistinguishable from two identical prompt
versions.

### `workers/admin/src/frontend/components/VersionEditor.tsx:55`

Rule `ts-catch-clause-tsx`, site `d8a81765f72b9a23`, lines 55-57.

VersionEditor.tsx:55 - empty catch with the comment 'Error handling is done by parent'.
The only parent is PromptDetail.handleSaveNewVersion (PromptDetail.tsx:47-58) whose
catch does nothing but `throw err; // Let the VersionEditor handle error display`. The
delegation is circular: neither side sets error state. A failed updatePrompt leaves the
editor open with saving=false and no message, so a failed save is indistinguishable from
a user who simply has not pressed save yet.

### `workers/admin/src/frontend/lib/api.ts:506`

Rule `ts-catch-clause-ts`, site `a664c577edb342c7`, lines 506-509.

api.ts:506 testApiKey - returns false on ANY throw. fetchWithAuth throws on a missing
key, on 401, and fetch itself rejects on a network or DNS failure. Login.tsx:27-35 maps
false to 'Invalid API key'. A server outage or offline browser is therefore reported to
the operator as a wrong key. The catch comment only accounts for the 401 path.

### `workers/admin/src/frontend/pages/Feedback.tsx:308`

Rule `ts-catch-clause-tsx`, site `63bb84c268d89b4d`, lines 308-310.

Feedback.tsx:308 CorrectionDetailPanel - the comment says 'malformed JSON; show raw' but
no raw rendering happens. toolHistory stays [] and the render guard at line 349
`toolHistory.length > 0` removes the whole Tool History section. A correction whose
recent_tools_json is corrupt looks identical to one that recorded no tools, and the
stored payload is never shown.

### `workers/admin/src/frontend/pages/PromptDetail.tsx:73`

Rule `ts-catch-clause-tsx`, site `18b9ca1517bc3744`, lines 73-75.

PromptDetail.tsx:73 handleShowDiff - bare `catch {}` with the comment 'Fetch failed - no
action needed'. setShowDiffModal(true) is skipped and no error state is set, so a failed
getPrompt leaves the Compare button looking like a dead control: the spinner stops and
nothing happens. Indistinguishable from a click that was never registered.

### `workers/admin/src/frontend/pages/QaAgent.tsx:179`

Rule `ts-catch-clause-tsx`, site `581c7ddc8196cc90`, lines 179-181.

QaAgent.tsx:179 loadPrompt - bare `catch { setPromptContent(null) }`. null is the same
value the effect sets at line 170 when no prompt key resolves, and the render at 337-343
turns any falsy promptContent into 'Prompt not available'. A 500 from getQaPromptContent
is therefore presented identically to a scenario that legitimately has no prompt, and
the operator testing a QA prompt sees no indication the fetch failed.


## Rule sources

The rule paths above point into a session scratch directory that does not survive the
session. The rule text is therefore reproduced here so the completeness claim stays
readable after the files are gone.

`ts-catch-clause-ts` (`language: typescript`, matches `.ts` only):

```yaml
id: ts-catch-clause-ts
language: typescript
rule:
  kind: catch_clause
```

`ts-catch-clause-tsx` (`language: tsx`, matches `.tsx` only):

```yaml
id: ts-catch-clause-tsx
language: tsx
rule:
  kind: catch_clause
```

No narrowing was applied. The rule is the broadest one available for the question -
every catch clause in the scope is a candidate - so no catch clause in `workers/admin/src`
was excluded from judgement by rule shape.

## Rule verification, before the census

- **Known positives.** `worker/health/handlers.ts` has catch clauses at lines 42, 71 and
  104 (`grep -n 'catch'`). The `typescript` rule matched all three, at exactly those lines.
- **Known negatives.** `worker/llm-usage/handlers.ts`, `worker/middleware/auth.ts` and
  `frontend/lib/auth.ts` contain no catch clause. The rule returned 0 matches for each.
- **AST, not text.** A probe file containing the word `catch` in a line comment, the
  string `"} catch (err) { return []; }"` in a string literal, and a `try/finally` with no
  catch, produced 0 matches.
- **No-binding form covered.** A probe `try { risky(); } catch { return []; }` matched, so
  the rule does not miss the `catch {}` form. This matters: 5 of the sites in the sweep,
  including 2 of the 6 violations, use that form.
- **Count sanity.** 22 matches in `.ts` + 34 in `.tsx` = 56. An independent
  `grep -rn '} catch'` over the same tree returns 56 lines. The count is neither zero nor
  equal to the file count (49 source files).
- **Language split.** `language: typescript` matched only `.ts` files and `language: tsx`
  only `.tsx` files, confirmed by bucketing the JSON output by extension. A single
  `typescript` rule would have silently omitted all 34 `.tsx` sites, which is where every
  one of the 6 violations lives.

## Limits of this claim

- The claim covers **catch clauses only**. A failure swallowed without a `catch` - an
  unawaited promise, a `?? []` on a result that already encodes failure, a `response.ok`
  check that is simply absent, a `.catch()` promise handler - was never a candidate. (No
  `.catch(` handler exists in this scope; checked by grep, so that particular gap is empty
  here.)
- Verdicts were reached by reading each catch clause **and** the render or response path
  its value reaches. Where a value crossed a component boundary
  (`VersionEditor` -> `PromptDetail`, `api.testApiKey` -> `Login`) both sides were read.
- Two sites are recorded `pass` despite being implicated in a violation, because the
  swallow happens elsewhere and double-counting would overstate the finding:
  `PromptDetail.tsx:55` (a no-op `catch (err) { throw err }` whose comment misdescribes
  what VersionEditor does with the rethrow) and `qa-agent/handlers.ts:111` (returns a
  correct 500 that the frontend then discards).

## Pattern worth copying

`UsersAndProjects.tsx:253-255` is the only site in the scope that orders its render
branches correctly:

```
projectsLoading ? spinner
: projectsError ? <error>
: userProjects.length === 0 ? <empty state>
: <list>
```

The error branch is checked *before* the empty branch. Every violation in this report is
the same mistake: a failure path that writes the empty or default value and then falls
through to a branch that means "nothing here".
