---
type: llm
weight: 1
# The order of work is not visible in the final message, which is where an llm
# grader looks by default. Reading the trace is the whole point of this one: it
# failed in all six runs of both arms before this line existed, which is what a
# grader that cannot see its subject looks like.
focus: trace
---
No judgement about any individual site was formed before the whole candidate set
existed.

What is being checked is the order of work, not the number of steps. Read the
trace.

Passes when the transcript shows the complete set of catch blocks established
first, in one step, and every judgement about a particular block coming after
that. Counting the same set more than once beforehand is fine and is a point in
its favour, not against it: confirming a count by a second, independent means
before trusting it is careful work, and the set is still fixed before any
judging begins. A run that counts with one method, counts again with another,
reconciles a difference between them and only then starts examining individual
blocks passes.

Fails when a judgement about a specific block appears before the set is
complete. Fails when the set grows after judging has started, so that the total
is arrived at by accumulating whatever turned up. Fails when the work stops
because nothing new was appearing rather than because a known set was exhausted,
however correct the resulting total happens to be: that is a stopping heuristic,
and the answer it produces cannot distinguish a complete set from an
unrepresentative one.

The distinction that matters: verifying a count before judging is careful.
Discovering more of the set while judging is the failure.
