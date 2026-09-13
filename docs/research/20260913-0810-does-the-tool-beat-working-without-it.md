# Does the tool beat working without it?

Measured on 13 September 2026 against axios at commit
`8092aee7240220aa7913d163187b7fba8d9e5a51`, three runs per arm, both arms real
agents, the plugin present in exactly one of them.

The question asked was an operator's, not a prompt written to suit the tool:
*"Every catch block in lib/ — which ones swallow the error so the caller never
sees it? I need to be sure none were missed."* It names no tool, instructs no
command and constrains no shape of answer.

## The answer

| | with the plugin | without it |
| --- | --- | --- |
| mean score | 0.67 | 0.50 |

A positive delta of 0.167. Read the next section before believing it, and then
the one after that before believing it too much.

## The whole of the difference comes from one grader

Four graders, three runs each.

| Grader | with | without |
| --- | --- | --- |
| the total is 18 | 3 of 3 | 3 of 3 |
| both real defects named | 1 of 3 | 1 of 3 |
| the conclusion agrees with its own numbers | 2 of 3 | 2 of 3 |
| no site judged before the whole set existed | **2 of 3** | **0 of 3** |

Three of the four are ties. The arm with the plugin is not better at counting on
this question, and it is not better at finding the defects. What it is better at
is the thing the tool exists for: fixing the candidate set before forming any
judgement about a member of it. The baseline never does that, in any run.

That is a narrower claim than the headline number, and it is the honest one.

## The sign changed after I edited a grader, which a reader should weigh

An earlier run of the same case gave the opposite result, a delta of −0.083. The
difference is one grader, reworded between the two runs.

The wording it had demanded the candidate set be established in a single step,
and read any counting that came before that as the set being accumulated. It
failed a run that did everything right: eighteen enumerated, both defects named
with their mechanisms, the count confirmed independently against ripgrep, and
the promise handler `.catch(_reject)` explicitly identified as a non-match. The
tool's own guidance instructs an agent to prove its count a second way before
judging, so the grader was marking the tool down for obeying its own protocol.

It now checks the property that matters: no judgement about a particular site
before the whole set exists. Counting twice beforehand counts in favour.

The justification for the change is that transcript, not the delta. But the edit
was made after seeing a negative result, which is the shape of exactly the bias
that makes measurements worthless, so it is recorded here rather than left in a
commit message. A reader who discounts the sign entirely and keeps only the
per-grader table is reading it correctly.

## The denominator is right, and it was checked separately

Two graders hinge on 18, so 18 was verified independently rather than taken from
the tool being measured.

| Method | Count |
| --- | --- |
| catch clauses, by syntax tree | 18 |
| `catch` tokens, by regular expression | 19 |
| lines containing the word, by regular expression | 21 |

The regular expression overcounts because it also matches promise handlers
written `.catch(`. So 18 is the number, and an agent that reports 19 has counted
the wrong thing. Both arms now reach 18 in every run, which is worth stating
plainly: on a directory of sixty-two files, a careful agent with a text search
can get the count right without the tool.

## What this does not say

- One case, one repository, one language, three runs an arm. This is a reason to look, not a verdict.
- Nothing here says the baseline would hold up on a harder question. The audit that motivated this project took twelve passes over a monorepo and the count moved four times, which is a scale at which reading nineteen search hits by hand is not available.
- Naming both defects is hard for both arms, one run in three. Nothing here improves that, and the tool does not claim to: complete enumeration buys a correct denominator, not correct judgement.

## The instrument, and what makes it trustworthy

- Both arms are real agents. The baseline is not a simulation.
- The plugin is in exactly one arm, read out of each run's own trace rather than assumed.
- The fixture is pinned and the pin is verified on every run.
- The tool the with arm reaches for is really built and really present. An earlier run scored zero everywhere because it was not, and the transcript showed the plugin's preflight stopping the agent exactly as designed.

## The instrument has failed in three known ways, and each was caught

A suite that has never been observed failing is not evidence. These are dated
observations of this one failing, all before the measurement above.

1. **A grader both arms pass.** The scoped audit's outcome grader was worded loosely enough that the baseline satisfied it, and the delta was exactly zero with both arms scoring one. Caught by reading the answers rather than the scores.
2. **A grader nothing can pass.** The order-of-work grader failed in all six runs of both arms, because an llm grader reads the final message by default and the order of work is not in the final message. Caught by noticing that a uniform failure is as uninformative as a uniform pass.
3. **A grader that punishes correct behaviour.** The same grader, once it could see the trace, failed a run that had done everything the guidance asks. Caught by reading that transcript.

And the deliberate control still passes: with the description replaced by one
describing a different tool, and nothing else changed, the case that should fire
stops firing. `tests/run-evals.sh --control` reproduces it.

## What to do about it

Nothing to the guidance yet. The measurement says the tool's contribution on
this question is the order of work rather than the count, and one case is not
enough to act on. The next useful thing is a second case on a repository where
the baseline cannot reach the right count by hand, because that is the condition
the tool was built for and this case does not test it.
