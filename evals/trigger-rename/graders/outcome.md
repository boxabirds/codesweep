---
type: llm
weight: 1
---

The answer recognises that a rename depends on which declaration each `save`
refers to, and that matching the name alone cannot tell one `save` from
another. A good answer says so, and points at something that resolves symbols,
such as a language server or the editor's own rename.

Fails if the answer proposes to find every occurrence of the name and change
them, without noting that same-named things elsewhere would be changed too.
This is the case where acting on a name-matched set corrupts code rather than
merely misinforming, so a confident plan with no caveat is the worst outcome.
