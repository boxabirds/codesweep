---
type: llm
weight: 2
focus: trace
---
The `.tsx` files were covered, not only the `.ts` files.

This is the trap in this tree and it is worth more than one point. Of the 123
places, 50 are in `.tsx` files. An approach that reaches only `.ts` finds 73 and
has no way of knowing it is short, because the files it missed are still there,
still full of the thing being counted, and nothing about the result looks
incomplete.

Read the trace, not only the final message. What is being checked is whether the
markup files were actually reached.

Passes when the transcript shows both kinds of file being covered, whether by
one method that handles both or by two that are combined. Passes if the run
first covered only `.ts`, noticed the gap, and went back for the rest: noticing
is the skill being measured.

Fails when only `.ts` files were ever examined. Fails when `.tsx` files are
mentioned as an afterthought or a caveat without being counted. A caveat is not
coverage, and an operator about to edit every site cannot act on one.
