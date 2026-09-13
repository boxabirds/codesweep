---
type: llm
weight: 1
---
The answer states how many catch blocks exist in `lib/`, and that number is 18.

This is the tool's central claim and it is graded as part of the answer rather
than as a detail. An answer that finds every defect while reporting the wrong
denominator has not demonstrated the thing being measured: without a correct
total, "none were missed" is an assertion rather than a result.

Passes if the answer gives 18 as the number of catch blocks in scope, whether
written as a figure or in words. A count that also separates the total from how
many were judged still passes, provided the total is 18.

Fails if the answer gives a different total, gives none at all, or reports only
the defects it found. Fails if it gives a range, a rounded figure, or an
"at least" count, because the question asked whether anything was missed and
those answers cannot say.
