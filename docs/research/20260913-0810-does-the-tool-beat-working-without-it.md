# Does the tool beat working without it?

Measured on 13 September 2026 against axios at commit
`8092aee7240220aa7913d163187b7fba8d9e5a51`, three runs per arm, both arms real
agents, the plugin present in exactly one of them.

The question asked was an operator's, not a prompt written to suit the tool:
*"Every catch block in lib/ — which ones swallow the error so the caller never
sees it? I need to be sure none were missed."* It names no tool, instructs no
command and constrains no shape of answer.

## The answer, which is not the one the tool would like

| | with the plugin | without it |
| --- | --- | --- |
| mean score | 0.50 | 0.58 |

The delta is **negative**. On this question, on this repository, the tool does
not beat working without it.

## Where it wins and where it loses

Four graders, three runs each.

| Grader | with | without |
| --- | --- | --- |
| the total is 18 | 1 of 3 | 3 of 3 |
| both real defects named | 1 of 3 | 2 of 3 |
| the conclusion agrees with its own numbers | 3 of 3 | 2 of 3 |
| the candidate set was fixed before judging began | 1 of 3 | 0 of 3 |

The tool arm is more internally consistent and is the only arm that ever fixes
its candidate set before forming a judgement, which is precisely what the tool
is for. It loses on the two graders that matter most to an operator: how many
there are, and which ones are the problem.

## The denominator is right, and it was checked separately

Both of those graders hinge on 18, so 18 was verified independently rather than
taken from the tool being measured.

| Method | Count |
| --- | --- |
| catch clauses, by syntax tree | 18 |
| `catch` tokens, by regular expression | 19 |
| lines containing the word, by regular expression | 21 |

The regular expression overcounts because it also matches promise handlers
written `.catch(`. So 18 is the number, a text search gives 19, and an agent
that reports 19 has counted the wrong thing. That is the difference the
documented journey said should be explained rather than tolerated.

The baseline arm reports 18 in all three runs. A careful agent with a text
search can reach the right number here by reading its own hits and discarding
the ones that are not blocks.

## What this does and does not say

It says the tool did not help on this question. It does not say the tool is
useless: one case, one repository, one language, three runs an arm. A negative
result on a sample this size is a reason to look, not a verdict.

It also does not say the baseline would hold up on a harder question. Axios's
`lib` directory is sixty-two files. The audit that motivated this whole project
took twelve passes over a monorepo and the count moved four times, which is a
scale at which reading nineteen search hits by hand is not available.

What it does establish is that the suite can produce a result the tool's author
did not want, which is the only kind of measurement worth having. Two earlier
versions of this case could not: one where both arms passed a grader worded so
loosely that nothing separated them, and one where a grader failed in all six
runs because it was reading the final message when its subject was the order of
work.

## The instrument, and why it can be trusted this far

- Both arms are real agents. The baseline is not a simulation.
- The plugin is present in exactly one arm, which is read out of each run's own trace rather than assumed.
- The fixture is pinned and the pin is verified on every run.
- The tool the with arm reaches for is really built and really present. An earlier run scored zero everywhere because it was not, and the transcript showed the plugin's preflight stopping the agent exactly as designed.
- The control passes: with the description replaced by one describing a different tool, the case that should fire stops firing.

## What to do about it

Nothing yet, and deliberately. The next thing is to find out *why* the tool arm
gets the count wrong, which is a question about what the agent does with the
guidance rather than about the tool's arithmetic: the tool's own census of that
directory returns 18 every time. Changing the guidance before knowing that would
be tuning against a number rather than fixing a cause.
