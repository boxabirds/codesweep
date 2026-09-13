# Where the context ceiling actually is

2026-09-13. The ledger's purpose is to disconnect the size of a job from the
size of the context window. That is a claim with a number in it, and the number
had never been measured. This is the measurement.

## Cost per site

Measured against axios `lib/` at the pinned fixture commit, with a rule
matching every call expression: 1037 candidate sites.

| | bytes | tokens (approx) |
| --- | --- | --- |
| one `next --limit 15` batch | 13,728 | ~3,800 |
| per site | 915 | ~254 |
| `manifest`, all 1037 sites | 35,766 | ~10,000 |

## The asymmetry that is the whole argument

Holding the **identity** of 1037 sites costs about 10k tokens. Holding their
**source** costs about 264k. The first fits in any window. The second fits in
none.

The ledger sits in that gap. The complete set is cheap and stays addressable;
the expensive part is streamed in batches of fifteen at roughly 3.8k tokens
each, and the verdicts accumulate on disk rather than in the transcript. What
is resident at any moment is one batch, not the job.

So the ceiling on a job stops being the window and becomes the disk.

## Where the ceiling falls

| sites | source payload | verdict |
| --- | --- | --- |
| 123 (the private eval) | ~31k tokens | comfortably inside a 200k window |
| ~390 | ~100k tokens | half the window, the practical limit |
| 1037 (axios lib) | ~264k tokens | exceeds a 200k window outright |

The phase change is somewhere around 300 to 400 sites, before any allowance for
reasoning, notes or edits.

## What this says about the evidence so far

Every eval case in the suite sits below the ceiling. The largest is 123 sites
at roughly a third of it, in a single sitting of 34 turns, with no compaction.

That is the regime where the ledger is pure overhead. Below the ceiling an
agent can hold the whole job in context and the ledger buys discipline and an
audit trail, which are real but modest. Above it the comparison is not one
score against another: the arm without the tool cannot complete the job at all,
and what it produces instead is a confident partial answer, which is the exact
failure this product exists to remove.

So the ambiguous ablation result was not weak evidence about the product. It
was evidence gathered in the one regime where the product cannot show anything.

## What an eval that tests the claim looks like

Not a grader rewrite. A different case.

- A candidate set above the ceiling, so the whole job cannot be resident. axios
  `lib/` at 1037 sites is available and already pinned. Something in the 400 to
  600 range would be above the ceiling and still drainable.
- A question needing a judgement per site, so the queue must actually drain
  rather than being answered by counting.
- Turn limits high enough to permit roughly one batch per turn. At fifteen per
  batch, 1037 sites is about seventy batches, which no existing case allows.
- Both arms, and the assertion is completion against abandonment, not score
  against score.
- The grader that matters: does the claimed coverage match the coverage that
  actually happened. An answer that examined 200 of 600 sites and says so has
  succeeded. An answer that examined 200 and implies 600 has failed, however
  good its prose.

That last grader is the one worth building carefully, because it is the only
one that measures the thing the product is for.

## Claims still without coverage

The ledger makes five claims. This measurement bears on three of them and
settles none.

- Nothing is skipped, because the queue forces a verdict on every candidate.
- The work survives compaction, because state is on disk and site identities
  are stable.
- `recheck` catches a site whose code changed after it was judged.

All three exist and are covered by the CLI suite as mechanisms. None has an
eval that exercises it as a workflow, and the first two only bite above the
ceiling, where nothing has ever been run.
