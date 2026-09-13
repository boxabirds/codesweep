# Does the skill still fire on the right questions after the rename?

Run on 13 September 2026, against the renamed plugin, with the workspace staged
from the pinned third-party fixture.

## The round

| Case | The question asked | Should fire | Skill called | Runs |
| --- | --- | --- | --- | --- |
| trigger-scoped-audit | which catch blocks in lib/ swallow the error | yes | once | 1 |
| trigger-caller-finding | find every caller of createProject | no | never | 3 |
| trigger-rename | rename the save method everywhere it is used | no | never | 3 |
| trigger-single-literal | where is the string DEPRECATED_API_URL used | no | never | 3 |

Ten runs, and the trigger grader passes on all four cases. It fires on the
question whose answer is a complete set, and declines the three that need a
language server or a plain search.

## The control, which is the part that makes the round evidence

A round that has never failed is not evidence. With the description replaced by
one describing a different tool entirely, and nothing else changed, the audit
case stops firing: `Skill called 0x (expected 1..∞)`. So the round is capable
of failing, and its passing means something.

`evals/run.sh --control` reproduces this.

## Why there is no before-and-after comparison

The story asked for the round before the substitution and again after, to
confirm it fires on the same questions either side. That comparison was not run,
because there is nothing in it to compare: the description contains no
occurrence of the tool's name and never did. Only the `name:` field in the
frontmatter changed. That is not an argument, it is checked, by
`the_trigger_sentences_in_the_guidance_are_untouched` in
`crates/resweep/tests/surface.rs`, which compares the description literally
against the version from before the port and fails on any difference.

Running the earlier round would have been running the same text twice.

## What this round does not say

The trigger grader is the only one passing. Every outcome grader fails,
including on the case where the skill did fire and the tool was used. That is a
question about whether the answer produced is any good, not about whether the
tool is reached for, and it belongs to the story that measures the tool against
working without it. It is recorded here because a reader seeing four green
trigger rows should not infer more than they say.

## Running it at all took a fix

The harness refuses a plugin containing any file with more than one name, and
cargo leaves tens of thousands of hard-linked object files under `target/`. Run
in place, every case failed before an agent started, with an error about case
definitions that points nowhere near the cause. `evals/run.sh` stages a copy
without the build output and checks for any remaining hard link before starting.

This means the suite had never run. Anything written about it before today was
about cases nobody had executed.
