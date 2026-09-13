# Where these came from

Two real index files, produced by `@sourcegraph/scip-typescript` on 13 September
2026 during the reproduction recorded in
`docs/research/20260913-0726-resolution-layer-silent-incompleteness.md`.

The project was four TypeScript files under `src/`: `app.ts`, `document.ts`,
`report.ts` and `session.ts`. Two of them export an unrelated function called
`save`.

`healthy.scip` indexes all four.

`one-file-outside-the-program.scip` was produced from the same source tree with
`report.ts` still on disk but outside the `include` list in `tsconfig.json`. It
holds three documents. The occurrences of `session.ts/save()` fall from eight to
six. The indexer printed `done` and exited zero, and said nothing on either
stream.

They are committed rather than regenerated because the point of the pair is the
difference between them, and regenerating requires an npm install of an indexer
whose maintenance record is the weak link in this whole layer. A test that
skipped whenever that install was absent would skip on every machine that has
not done it, which is most of them.

They will go stale if the format changes. That is acceptable and is what the
reproduction note is for: it records how to make them again.
