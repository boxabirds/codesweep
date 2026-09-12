---
type: llm
weight: 1
---

This is the case the next piece of work proposes to change, so the baseline
matters more than the verdict.

Today the right answer notes that finding every caller depends on which
declaration the name refers to, and that a text or syntax match cannot tell one
`createProject` from another of the same name. Saying so, and pointing at
something that resolves symbols, passes.

A list of matches presented as the complete set of callers, with no statement
of what it cannot distinguish, fails. That is the failure this case exists to
detect, and it stays a failure whether or not the tool learns to answer the
question, because the caveat is the part that must survive.
