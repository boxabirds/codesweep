---
type: llm
weight: 2
---
The answer gives both numbers the question needs, and distinguishes them.

There are 123 places in this tree where `||` or `??` appears as an operator.
That is the candidate set and it is a syntactic fact. Not all of them supply a
fallback: some are boolean tests, `if (!key || !data)` and the like, where the
operator is combining conditions rather than defaulting. Deciding which is which
is a judgement, and the operator asked about the ones that supply a fallback.

Passes when the answer gives the candidate total of 123 and a judged total of
the subset that actually supplies a fallback, and makes clear which is which. An
answer of the form "123 uses of the operators, of which N supply a fallback,
the rest being boolean conditions" is exactly right, and the value of N is not
graded here: it is a judgement over 123 sites and reasonable readings differ.

Passes if the two numbers are given in either order or in different words, as
long as a reader can tell the enumerated set from the judged subset.

Fails when only one number is given, because then a reader cannot tell whether
the work covered everything and judged some, or covered only some. That
ambiguity is the whole thing this tool exists to remove.

Fails when the candidate total is not 123, since the enumeration is mechanical
and there is a right answer. Fails on a range or an approximation for that
total, and fails when a count of lines or of files is offered in place of a
count of places: a line holding two operators is two places.

This grader was rewritten after a run reported "123 total, 89 supply a
fallback", with the 34 boolean conditions named, and was marked wrong by a
version that demanded the bare figure 123. That run had given a better answer
than the grader knew how to ask for.
