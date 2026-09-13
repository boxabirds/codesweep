---
type: llm
weight: 2
---
The answer states how many places supply a fallback with `||` or `??`, and that
number is 123.

Weighted double, because it is the whole question. The operator said they are
about to change every one of them; a number that is wrong by ten means ten sites
silently keep the old behaviour, and a number given without confidence means the
work cannot start.

Passes on 123, written as a figure or in words.

Fails on any other total. Fails on a range, an approximation, or an "at least"
figure, because none of those answers the question that was asked. Fails if the
answer gives only a count of files, or a count of lines, rather than of places:
a line holding two operators is two places, and conflating them is the specific
error a text search makes here.
