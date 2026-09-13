# SCIP as ground truth, on a real codebase

2026-09-13. The question was why the syntactic layer was being validated
against hand-counted toy fixtures when a symbol indexer could supply ground
truth over a real tree. The answer was that nothing was stopping it and it had
not been tried. This is the trying.

Target: the admin worker of the private codebase, at the same commit the eval
pins (`581cd0ee`), in a throwaway worktree. 58 TypeScript files, 22 `.ts` and
27 `.tsx` under `src/` plus integration tests and configs.

## The indexer is healthy here, and cheap

`@sourcegraph/scip-typescript`, run over `workers/admin`:

| | documents | time |
| --- | --- | --- |
| without node_modules | 55 of 58 | 253ms |
| with node_modules (391 packages) | 55 of 58 | 765ms |

The two indexes cover the same documents. Installing the dependency tree
changed nothing about which files resolved, which matches the earlier toy
reproduction where uninstalling a dependency produced a byte-identical index.

The three files not indexed are `vite.config.ts`, `vitest.config.ts` and
`vitest.frontend.config.ts`. All three sit at the package root, outside
`include: ["src/**/*", "tests/**/*"]`. Correctly excluded, not silently
dropped.

`resolution::documents_in_scip` recovers all 55, so `check()` returns
`Health::Complete`. The crude byte-scraping reader, which exists to avoid a
protobuf dependency for one field, was verified against a full protobuf parse
of the same 1.1MB index and agreed exactly.

A wrong intermediate result is recorded here because it cost time. A Python
transliteration of that reader reported 48 documents and 10 missing, including
seven real `.tsx` test files, and that looked like a serious defect in shipped
code. It was a bug in the transliteration. The Rust is correct. Check the
shipped implementation before believing a reimplementation of it.

## Head to head, against 18,231 real occurrences

Ground truth is the index's reference occurrences for a symbol, excluding its
definition. The comparison is resweep's syntactic candidate set for the same
question, using `pattern: NAME($$$ARGS)` rules for `typescript` and `tsx`.

### A plain function: `fetchWithAuth`

Defined at `src/frontend/lib/api.ts:16`, 30 references, all in that file.

| | sites |
| --- | --- |
| SCIP references | 30 |
| resweep | 30 |
| lines SCIP has and resweep does not | 0 |
| lines resweep has and SCIP does not | 0 |

Line for line, exactly. On a plain named function call, the syntactic layer is
not an approximation of the symbol layer. It is the same answer.

### A React component: `DiffModal`

Defined at `src/frontend/components/DiffModal.tsx:37`, 21 references across 4
files.

| | sites |
| --- | --- |
| SCIP references | 21 |
| resweep | 0 |

None of the 21 is a call. They are `<DiffModal ... />` elements, an
`import DiffModal from './DiffModal'`, and the name inside `describe('DiffModal')`.
A rule asking for `DiffModal($$$ARGS)` is asking the wrong question, and it
returns zero with no indication that the question was wrong.

An operator could write more rules: one for the JSX element, one for the import
clause. The point is that they have to know to, for this symbol, and the
indexer simply knows. That is the value of the symbol layer stated as a number
rather than as an argument.

## What this settles

The syntactic layer's accuracy is no longer a question on plain call sites. It
was 30 of 30 against a real index on a real codebase, not 3 of 4 against a
fixture somebody hand-counted.

Cross-file symbol reuse in this codebase is thinner than expected. Exactly one
symbol has references in three or more files. It is mostly React components and
per-file handlers, which is worth knowing before building a caller-finding
feature around this tree.

## What this cannot settle

SCIP indexes symbols. The flagship eval question is how many places supply a
fallback with `||` or `??`, and an operator has no symbol, so there are no
occurrences to compare against. That question can never be validated this way.
It is not a gap to close: for pattern questions the syntactic layer is itself
the complete answer, and the judgement of which candidates supply a fallback is
genuinely a judgement.

So the oracle splits by question type:

- **Caller and rename questions.** SCIP is ground truth, available now, under a
  second, and the comparison is exact.
- **Pattern and operator questions.** No symbol oracle exists. Completeness is
  syntactic and provable; the subset judgement is not.

## Reproducing

    git worktree add --detach <tmp> 581cd0ee
    npm install --no-save @sourcegraph/scip-typescript
    cd <tmp>/workers/admin && scip-typescript index --output <tmp>/admin.scip

No `npm install` of the project's own dependencies is needed; it changes
nothing. The index is of a private codebase and is not committed.
