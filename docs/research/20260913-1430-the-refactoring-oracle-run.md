# Running the Refactoring Oracle: the parser is level with grep

2026-09-13. The gate was: does this tool produce a better list of affected
sites than a host agent already gets from its own primitives? The Refactoring
Oracle supplies the answer key. This is the run.

Harness in `tests/bench/oracle/`, results in `results-27-cases.txt`.

## Method

The oracle records validated refactorings as prose against a commit, with no
source locations. The answer key is derived: for a `Rename Method` instance,
the sites that must change are the lines in the parent commit that the commit
removed and which mention the old name.

27 distinct rename cases across mockito, graphhopper and apache/drill. 398
ground-truth sites. Compared against `grep -rnw <oldname>`, which is what a
host agent reaches for unaided.

## Result

| | recall | precision | cases scored exactly right |
| --- | --- | --- | --- |
| `grep -w` | 100.0% | 90.2% | 21 of 27 |
| resweep | 98.5% | 90.7% | 20 of 27 |

Level. Recall differs in 2 of 27 cases, precision in 2, in opposite directions.
Against a 398-site total the difference is 6 sites one way and 3 the other.

**For rename-shaped questions in Java, the parser buys nothing over grep.**

## The wrong turn, because it cost an hour and would have been a false finding

The first run reported resweep at 92.5% recall and 85.2% precision, losing to
grep on both axes, and it looked like a decisive negative. It was a harness
defect.

resweep reports the **start line** of a match. The ground truth and grep both
report the line **containing the identifier**. For a call spanning several
lines those differ, and the harness counted each such site as two errors: a
miss and a false positive. Because grep's output is line-identical to the
ground truth by construction, the defect could only ever penalise resweep.

Caught by checking whether disagreements clustered: on the largest case all 5
missed sites lay within 3 lines of a resweep hit. Re-scored with a greedy
one-to-one match within the same file and 3 lines, applied to both tools
equally. The mockito case was unchanged by the tolerance, which is the control
that shows it is not simply inflating everything.

Never report a comparison where one arm's output format matches the answer
key's format by construction and the other's does not.

## What both tools get wrong, and it is the same thing

Neither reaches 100% precision. Both sit near 90%, and the residual is
same-name collisions.

Verified in detail on one case. mockito renamed `describeTo` to `describe`.
33 sites must change. Four must not: `ArgumentMatcher.describeTo` and a test
implementing it are Hamcrest's method, which survives the commit untouched.
Nothing syntactic separates them. grep flagged all 4, resweep flagged 2.

That roughly 10% is what a symbol index removes, and it is the clearest
measured case for the resolution layer in this project so far. It is verified
on one case and assumed, not verified, across the other 26.

## resweep's 6 missed sites are comments

On the mockito case the 3 misses are all Javadoc:

```
* ...an hamcrest matcher with predefined describeTo() method.
* @see org.mockito.ArgumentMatcher#describeTo(org.hamcrest.Description)
```

Earlier today "comments and strings are correctly excluded" was recorded as a
good property of the parser. For a rename it is a defect, because a rename has
to update documentation, and the developer did.

This is exactly what IntelliJ's rename preview solves by putting "In Strings,
Comments, and Text" in its own reviewable category rather than excluding it or
mixing it in. The fix is a categorisation choice, not analysis.

## What this does and does not settle

Settles: the syntactic layer is not the differentiator for symbol-rename
questions. A host agent with grep gets the same answer. Any claim that this
tool finds renames better is unsupported.

Does not settle: anything about pattern-shaped questions, which grep cannot
express at all and which no language server has a concept for. Those remain
the only measured gap and this benchmark cannot test them, because the oracle
catalogues named refactorings and "every place supplying a fallback with `||`"
is not one.

Also untested: TypeScript and JavaScript. The oracle has 550 Java commits and
25 JavaScript ones. This is a Java result.

## Reproducing

    tests/bench/oracle/drive.sh

Clones mockito, graphhopper and drill, runs every distinct rename case, prints
one row per case as `repo|name|truth|grep_hit|grep_wrong|grep_miss|rs_hit|rs_wrong|rs_miss`.
