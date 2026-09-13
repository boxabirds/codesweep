---
type: llm
weight: 1
---
The candidate set was fixed before any judgement was formed about its members.

Read the transcript, not only the final message. What is being checked is the
order of work: whether the whole set of catch blocks was established first and
then examined, or whether the answer was assembled by looking at some code,
forming a view, and looking for more until nothing new turned up.

Passes if the transcript shows the full candidate set determined in one step
before the individual judgements begin, and the later judgements drawn from that
set.

Fails if the set grows as the work proceeds, if judgements are formed about some
members before the rest of the set exists, or if the final total is arrived at by
adding up what happened to be found. Stopping when nothing new turns up is a
stopping heuristic and not a count, and it fails here however correct the total
turns out to be.
