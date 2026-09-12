# Where this crate came from

Copied from `codemine`, the operator's own workspace, at commit
`64f0bf1238e2dc7953c63a4f5d06c703603fb267`, path `crates/scanner-core`. Copied rather than depended on,
because codesweep is self-contained: a path dependency on a sibling checkout
would make this repository unbuildable for anyone who does not also have that
one, and would couple two projects that have no reason to move together.

The copy is of the committed tree, not the working tree. The source workspace
had uncommitted work in progress adding embedding storage to `SymbolIndex`;
that is not vendored here, because vendoring somebody's unfinished change is a
worse starting point than vendoring their last finished one.

## What is deliberately not carried over

- `scanner-py`, the pyo3 binding. Its consumer is a Python package in the
  source workspace and nothing here calls it. Its absence is why this copy is
  free to change the shape of `SymbolIndex` without breaking a caller.
- The bincode caches written by that workspace. Nothing here reads or writes a
  persisted index, so the on-disk encoding is not a compatibility surface.

## Divergence

This copy is expected to diverge and there is no plan to merge back. Changes
made here are made for codesweep. If a fix here is also wanted there, it is
carried across by hand and by choice.
