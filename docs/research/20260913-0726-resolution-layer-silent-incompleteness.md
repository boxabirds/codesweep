# The exact layer's failure mode, reproduced on purpose

Measured on 13 September 2026. This is the first piece of work the deferred
resolution story asks for, and it is done before any design because the design
is supposed to be built on it rather than on documentation.

The claim under test is the one the story calls the trap: when a project does not
fully resolve, the machinery that produces exact answers does not refuse. It
produces a result that is smaller than the truth and looks exactly like a
complete one.

## The setup

`@sourcegraph/scip-typescript`, installed locally, against a four-file
TypeScript project built for the purpose. Two unrelated modules each export a
function called `save`. Two other modules use them: one calls both, the other
calls only the first.

## First, the thing that works

The two same-named functions are genuinely told apart. The index holds them as
separate symbols:

```
scip-typescript npm twonames 1.0.0 src/`session.ts`/save().     8 occurrences
scip-typescript npm twonames 1.0.0 src/`document.ts`/save().    5 occurrences
```

A name-matching layer returns both sets for either question. This one does not,
which is the whole reason the exact layer is worth building.

## Two breakages that did nothing

Both were tried first and neither reproduced the failure, which is worth
recording so nobody repeats them.

Uninstalling the dependency changed nothing: the index was byte-identical, 3681
bytes either way. The dependency was used for a local variable and the local
files still resolved among themselves.

Breaking a file's imports, by importing a name from a module that does not
exist, also changed nothing that mattered. The index grew by 32 bytes and every
`save` count stayed the same. TypeScript recovered from the bad import and kept
the rest of the file's references.

So the failure is not simply "something is broken". It is narrower and worse.

## The breakage that reproduced it

The project's `tsconfig.json` was narrowed so that one of the four files,
`report.ts`, fell outside `include`. The file was left on disk, untouched.

| | documents indexed | occurrences of `session.ts/save()` |
| --- | --- | --- |
| healthy | app, document, report, session | 8 |
| one file outside the program | app, document, session | 6 |

The tool printed `done` and exited zero. No error, no warning, nothing on
standard error. An answer built from the second index reports two call sites
where there are three, and nothing about the answer says so.

That is the defect this product already shipped once at a lower altitude, when
one family of source files silently contributed nothing to an audit. Here it
would be worse, because the answer carries the word exact.

## The detection signal, and why it has to be this one

The requirement is explicit that detection must not rely on the preparation step
reporting failure, because it reports success. The reproduction confirms that:
exit zero, both times.

The signal is in the artefact. The index names the documents it contains. The
files are on disk. In the broken run `src/report.ts` is on disk and absent from
the index, and comparing the two sets is enough to know the answer is partial
and to name the affected file.

That comparison needs the same file walk the approximate layer already has, which
means the check is cheap and, more importantly, is built on a component that is
already tested against real repositories rather than on a new one.

## What this settles for the design

- The exact layer can tell two same-named things apart. That is measured, not assumed.
- Its incompleteness is silent and is not detectable from the exit status, the output, or the presence of an error.
- It is detectable by comparing the indexed document set against the source files on disk, before any answer is derived.
- Not every breakage causes it. A missing dependency and an unresolvable import both left the counts intact. The failure comes from files the program never included, which is why a check on "did anything fail" would not have caught it and a check on "is every file present" would.

## What this does not settle

- Nothing here says how big the gap gets on a real repository. The project was four files.
- Nothing here covers any language but TypeScript. Each indexer will have its own version of this and each needs its own reproduction.
- Nothing here measures how long indexing takes on a repository worth indexing, and the requirement that preparation cost be visible to the operator exists because it is slow enough that silence reads as a hang.
- `@sourcegraph/scip-typescript` remains the weak link identified earlier: six releases in three years.
