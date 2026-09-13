# What scip-typescript and the TypeScript language service actually resolve

2026-09-13. Salvaged from one subagent that completed before a runaway fan-out
was killed. Its findings come from reading the sources at pinned commits and
from running probes against the built tools, not from documentation. Recorded
here because they contradict an assumption this project was about to build on.

Provenance: `sourcegraph/scip-typescript` at `e22988d9` (v0.4.0, 2026-09-11),
`sourcegraph/scip` at `a279feba`, TypeScript v5.9.3 and `main`.

## The assumption this kills

The plan was to wire SCIP in as a confidence calibrator: run the syntactic
layer and the symbol layer over the same question, and treat the delta as the
guarantee. That plan assumed SCIP knows who calls what.

**SCIP has no call edges.** There is no caller-to-callee relation in the format.
The whole message list is `Index, Metadata, ToolInfo, Document, Symbol,
Package, Descriptor, Signature, SymbolInformation, Relationship, Occurrence,
Diagnostic`. Grepping the proto and docs for call graph terms returns one hit,
and it is a suggestion in a doc comment rather than a feature.

What exists is a flat list of `(range, symbol, symbol_roles)` per document plus
`enclosing_range` on definition occurrences. A call graph is something the
consumer reconstructs by geometric containment: for each reference, find the
innermost definition whose enclosing range contains it. Three losses are baked
into that reconstruction:

- A reference occurrence does not say whether it is a call, a read, or a type
  position. The `SymbolRole` bitset has `Definition, Import, WriteAccess,
  ReadAccess, Generated, Test, ForwardDefinition` and **no Call bit**.
- The callee recorded is the statically declared member, so interface-typed
  dispatch dead-ends at the interface.
- Some calls produce no occurrence at all. See below.

## The failure mode that matters most: `any` is silent

Measured on a probe project, decoding the emitted index:

| receiver | what scip-typescript emits |
| --- | --- |
| `g: Greeter` (interface) | one reference to `Greeter#greet()`, the interface method |
| `g: NominalGreeter` (class) | one reference to the class method |
| **`g: any`** | **no occurrence at all for `greet`** |
| `g: A \| B` (union) | **two** occurrences at the same range, one per declaration |
| `const s: Greeter = new StructuralGreeter()` then `s.greet()` | reference to `Greeter#greet()`; the actual runtime callee is never mentioned |

The `any` row is the one to sit with. The call is not marked unresolved. It is
**absent**. An index consumer counting occurrences sees nothing there and has
no way to distinguish "no call here" from "a call I could not type".

That is precisely the silent incompleteness this project exists to remove, and
it is present in the layer that was going to be the source of truth.

The union row is worth knowing too: the fan-out is an accident of the symbol
having two declarations, not deliberate dispatch resolution.

## SCIP relationships are nominal only

`SymbolInformation.relationships` carries `is_reference`, `is_implementation`,
`is_type_definition`, `is_definition`. scip-typescript does populate
`is_implementation`, and does record that `NominalGreeter#greet()` implements
`Greeter#greet()`.

But the traversal that finds them recurses only through `heritageClauses`, that
is `extends` and `implements`. A class with an identical shape and no
`implements` clause produced **zero relationships**. There is no structural
comparison anywhere in the indexer, and structural typing is TypeScript's
actual subtyping relation.

Relationships also point upward, stored on the derived symbol. Answering "who
implements this?" requires inverting the whole index.

## TypeScript's own language service is no better, and knows it

**Go to Implementation is a filtered text search.** It runs Find All References
and keeps the hits that look like implementations. The candidate positions come
from `text.indexOf(symbolName, ...)` in a loop.

Measured on TS 5.9.3: for `interface I { foo(): void }`, Go to Implementation
returns the `satisfies`, `as`, return-position and array-literal cases, and
does **not** return `class NoImplements { foo() {} }` — even when a
`NoImplements` instance is assigned to a variable of type `I` on the next line.

**The shipped Call Hierarchy is measurably wrong for interface dispatch:**

```
interface I { foo(): void }
class A implements I { foo() {} }
class B { foo() {} }
function caller(i: I) { i.foo() }
const b: I = new B()
function caller2() { b.foo() }      // runtime callee is B.foo
```

| | result |
| --- | --- |
| incoming to `A.foo` | `caller`, `caller2` — phantom, `caller2` never reaches it |
| incoming to `B.foo` | **nothing** — the only edge that actually executes |

Over-approximate on dispatch and unsound on structure, at the same time.

**The TypeScript team has known since 2016.** Issue #6388, "Allow structural
reference-finding in the language service", was filed in January 2016 and
closed in July 2020 for lack of feedback. Issue #11788, filed by a TS team
member against their own feature, was closed as not planned. On #25831 the then
dev lead explained the cost of doing it properly and concluded "find-all-refs
is not the place for it".

The clinching detail: #25831 was filed by the VS Code / Monaco lead because
this gap breaks the Monaco tree-shaker, and the workaround is
`build/monaco/monaco.usage.recipe` — a **hand-written file of fake references**
maintained by hand. Hand-maintained edge lists are the industry fallback.

## What this changes here

The symbol layer is still worth having. It is right about the things it is
right about, and 30 of 30 line-for-line agreement on a plain function was
measured separately today.

But it cannot be the oracle, and the confidence-calibration plan needs
rewriting. Specifically:

- Comparing syntactic against SCIP does not yield a call-graph delta, because
  SCIP has no call graph. Any comparison has to be built on reconstructed
  containment, with the three losses above stated.
- SCIP being silent on `any`-typed receivers means agreement between the two
  layers is not evidence of completeness in a codebase with `any` in it. Both
  layers can miss the same site for different reasons and agree.
- A site whose receiver is `any` is knowable and should itself be a reported
  residual. That is a genuinely useful signal and nothing in the ecosystem
  emits it.

## Sources

- scip-typescript `src/FileIndexer.ts` lines 89, 109, 169, 339, 375, 687
- scip `scip.proto` — SymbolInformation, Relationship, SymbolRole, Occurrence
- TypeScript v5.9.3 `src/services/findAllReferences.ts`; `main` `tsc/internal/ls/`
- https://github.com/microsoft/TypeScript/pull/10482
- https://github.com/microsoft/TypeScript/issues/6388
- https://github.com/microsoft/TypeScript/issues/11788
- https://github.com/microsoft/TypeScript/issues/25831
- https://github.com/microsoft/TypeScript/issues/58307
- https://github.com/microsoft/vscode/blob/master/build/monaco/monaco.usage.recipe

Claims above are from source-reading and probe runs by a subagent. The probe
scripts were in a scratchpad that is not preserved, so the measurements are
reproducible in method but not re-runnable as they stand.
