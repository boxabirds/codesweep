---
type: llm
weight: 1
---

The floor case. One literal with one spelling is answered exactly by a single
search, however many times it occurs, so the answer should just search and
report what it found.

Fails if the answer builds an enumeration ledger, or otherwise treats a
question a single search answers exactly as though its completeness were in
doubt. Reaching for heavier machinery here costs the user time and makes the
heavier machinery less credible when it is genuinely needed.
