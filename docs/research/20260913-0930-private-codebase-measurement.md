# Against the private codebase

Measured 13 September 2026, two runs per arm, against `workers/admin/src` from a
codebase that is not part of this project and is not committed here. Fifty-one
files. The question was the shape of the audit that motivated the whole project:
*"I want to route every place that supplies a fallback with `||` or `??` through
one helper. Before I start: how many such places are there in this tree, and how
confident should I be that the number is complete?"*

## The result

| | with the plugin | without it |
| --- | --- | --- |
| mean score | 0.58 | 0.33 |

A delta of **+0.25**, larger than the +0.167 measured on the public fixture. The
tool helps more here, which is what the earlier note predicted: this is a tree
where a text search cannot reach the answer by hand.

## Where it wins

| Grader | weight | with | without |
| --- | --- | --- | --- |
| the total is 123 | 2 | **0 of 2** | **0 of 2** |
| the markup files were covered | 2 | 2 of 2 | 1 of 2 |
| the confidence claim is about method | 1 | 2 of 2 | 2 of 2 |
| no site judged before the set existed | 1 | 1 of 2 | 0 of 2 |

The whole margin comes from two places. The tool arm covered the `.tsx` files
every time and the baseline missed them once. And the tool arm is again the only
one that ever fixes its candidate set before judging.

## The finding that matters more than the delta

**Nobody got the count right. Not one run of four, in either arm.**

The true number is 123, verified three ways before the case was written: the
syntax tree finds 123 across `.ts` and `.tsx`, a text search finds 120 lines or
124 operator occurrences depending how it counts, and the TypeScript rule alone
finds 73 because 50 of the sites live in markup files.

A trustworthy count is the tool's central claim. On a real private codebase, at
a size where the claim is supposed to matter most, it did not deliver one. The
tool's own census returns 123 every time it is asked; the agent using the tool
reported something else.

That gap is between the guidance and the tool, not inside the tool. It is the
most valuable thing this measurement has produced and it is the next thing to
chase.

## What this does not say

Two runs an arm. The delta is larger than the spread but not by much, and a
third run each would be worth having before quoting the number anywhere.

The scope is one directory of one private repository. Nothing here says how the
tool behaves across the whole monorepo, which is where the twelve-pass audit
happened.

## Getting here took four corrected mistakes, none of them about the tool

Recorded because each cost a full run and the next person should not repeat them.

1. The scaffold looked for the checkout under `$HOME`, which is remapped inside a run. Every case refused.
2. Exporting the path from the runner does not reach the scaffold: the harness scrubs the environment. A run whose runner printed the path still saw the variable empty.
3. Resolving the real home through the user database does work, and it is what the public fixture already did. Having fixed the first problem with it and the second at the same time, I attributed the success to the wrong one and removed it.
4. The default timeout of 300 seconds cut two runs off at twenty-six and twenty-nine turns, mid-work, scoring them zero for a reason unrelated to the answer.

Every one of those produced a plausible-looking zero. None of them was visible in
a score.
