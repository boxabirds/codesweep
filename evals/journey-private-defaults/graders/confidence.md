---
type: llm
weight: 1
---
The answer says how confident the reader should be, and the reason it gives is
about method rather than about feeling.

The operator asked two questions and the second one was how much to trust the
first. An answer that gives a number and stops has answered half.

Passes when the answer states what the count is complete with respect to, in
terms a reader can check: which files were examined, what kind of construct was
counted, and what would fall outside that. Saying the count covers a particular
set of file kinds and a particular syntactic shape is exactly right.

Passes when the answer names a genuine limitation, such as a construct that
supplies a fallback without using either operator.

Fails on bare confidence: "this is complete", "I checked everything", "I am
confident", with nothing a reader could verify or falsify. Fails when the answer
hedges without saying what the hedge is about, since that is as unusable as
false certainty.
