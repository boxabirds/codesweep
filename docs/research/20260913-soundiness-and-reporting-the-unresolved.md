# Who already ships "here is what I could not resolve"

2026-09-13. The question is narrow: is there prior art for a static analyser
that emits its own unresolved sites as a first-class, enumerated output — a
list of file:line locations it knows it failed on — rather than under-reporting
silently or burying the caveat in documentation?

The answer is yes, in more places than expected, and the interesting finding is
not the existence of prior art but its **shape**. Nearly everyone who does this
does it at one of two altitudes: a prose paragraph about feature classes, or a
per-location list hidden behind a debug flag. Almost nobody puts the residual in
the same artefact as the answer, at the same granularity as the answer, on by
default. The two exceptions are IntelliJ's rename preview and Soot's reflection
guards, and both are worth copying.

Provenance: all source claims are from files fetched from
`raw.githubusercontent.com` at `master`/`main` on 2026-09-13, kept in
`docs/research/probes/20260913-soundiness/` alongside a `README.md`
listing every probe. Maintenance status came from the GitHub API, not READMEs
(`docs/research/probes/20260913-soundiness/repo-status.sh`). The jelly measurement is re-runnable
from `docs/research/probes/20260913-soundiness/jellyprobe/`.

Web search quota was exhausted before this task started, so everything here came
from direct fetches: GitHub API and raw files, OpenAlex, Crossref, author
homepages, and the JetBrains YouTrack public API. Where that was not enough to
answer a question, it says so.

## 1. Soundiness: the manifesto asks for prose, not locations

**Livshits, Sridharan, Smaragdakis, Lhoták, Amaral, Chang, Guyer, Khedker,
Møller, Vardoulakis. "In Defense of Soundiness: A Manifesto." CACM 58(2),
February 2015.** [doi:10.1145/2644805](https://doi.org/10.1145/2644805),
pre-print at <https://yanniss.github.io/Soundiness-CACM.pdf>. Verified: DOI and
venue confirmed via OpenAlex; the three-page pre-print was read in full.

The core claim, in their words: "we are not aware of a single realistic
whole-program analysis tool ... that does not purposely make unsound choices."
A *soundy* analysis has "a sound core" — most features over-approximated — plus
"deliberately under-approximated handling of a feature subset well recognized by
experts."

What is given up, from their own table:

| language | commonly ignored |
| --- | --- |
| C/C++ | `setjmp`/`longjmp`, effects of pointer arithmetic, manufactured pointers |
| Java/C# | reflection, JNI |
| JavaScript | `eval`, dynamic code loading, data flow through the DOM |

The longer list on soundiness.org adds, for Java/C#: `invokedynamic`, runtime
code generation, dynamic loading, multiple class loaders, implicit invocations
(finalizers, class initializers, `Thread.<init>`), integer overflow, exceptions.
For JavaScript: `innerHTML` assignment, the `Function` constructor,
`setTimeout`/`setInterval`, computed properties, implicit conversions, getters
and setters, prototype semantics.

**On the question that matters to us: the manifesto asks for reporting, but at
the wrong altitude.** Its third message is "a call to the community to identify
clearly the nature and extent of unsoundness in static analyses. Currently, in
published papers, sources of unsoundness often lurk in the shadows, with caveats
only mentioned in an off-hand manner in an implementation or evaluation
section." The "Moving Forward" asks are all paper-level: papers should explain
implications, the community should provide guidelines on how to write papers.
There is one tool-level ask — compare analysis results against observable
dynamic behaviour — and that is a *validation* method, not an output channel.

The operationalisation confirms it. soundiness.org ships a **Soundiness
Statement Generator**: tick checkboxes for the features your analysis handles
unsoundly and it produces a paragraph beginning "This paper/project has been
done in the spirit of soundiness. When building practical program analyses, it
is often necessary to cut corners..." and ending with a list of *feature names*.
That is a disclaimer, not a residual set. It tells a reader that eval is
unhandled; it does not tell them that line 412 of `router.js` is one of the
sites where it mattered.

So: the manifesto argues for disclosure at the level of language features and
papers. It does not argue for enumerating source locations, and it did not ask
anyone to. Every implementation below that does enumerate locations went beyond
the manifesto rather than following it.

### The follow-up that did go per-location

**Christakis, Müller, Wüstholz. "Collaborative Verification and Testing with
Explicit Assumptions." FM 2012, LNCS 7436, pp. 132–146.**
[doi:10.1007/978-3-642-32759-9_13](https://doi.org/10.1007/978-3-642-32759-9_13).
Verified: authors and venue confirmed via Crossref; full PDF read from the first
author's page at <https://mariachris.github.io/Pubs/FM-2012.pdf>.

This is the closest research prior art and it predates the manifesto. The
technique makes a checker's compromises explicit **at the program point where
they are made**, using two new constructs: an `assumed <expr> as <name>`
statement, and an annotation on each assertion saying which named assumptions it
was verified under. Their worked example:

```csharp
Contract.Ensures(Contract.Result<int>() < 0);  // verified under a_na, a_ui0, a_ui1
// assumed c != d as a_na
if ((0 < c.value && 0 < d.value) || (c.value < 0 && d.value < 0)) {
    // assumed new BigInteger(-1) * new BigInteger(c.value) ==
    //         new BigInteger(-1 * c.value) as a_ui0
    c.value = (-1) * c.value;
}
```

Two properties we want: the residual is located, and it is consumed downstream —
a test generator reads the assumptions and targets exactly the unverified
executions. "All assumptions are expressed at the program points where they are
made."

The independent parallel line is **Beyer, Henzinger, Keremoglu, Wendler.
"Conditional Model Checking." FSE 2012.**
[doi:10.1145/2393596.2393664](https://doi.org/10.1145/2393596.2393664), arXiv
preprint <https://arxiv.org/pdf/1109.6926>. A model checker that fails normally
reports nothing despite having done real work; CMC reformulates it to return a
condition Ψ such that the program satisfies the spec as long as it stays in
states satisfying Ψ. The later reducer work (Beyer/Jakobs, e.g. **"FRed:
Conditional Model Checking via Reducers and Folders," SEFM 2020**,
[doi:10.1007/978-3-030-58768-0_7](https://doi.org/10.1007/978-3-030-58768-0_7))
turns the condition into a *residual program* — the unverified state space
rendered back as source you can look at. That is the strongest form of "here is
what I did not cover": not a list, an artefact.

### Does the located-residual idea pay off? One honest measurement

**Christakis, Müller, Wüstholz. "An Experimental Evaluation of Deliberate
Unsoundness in a Static Program Analyzer." VMCAI 2015.** PDF read from
<https://mariachris.github.io/Pubs/VMCAI-2015-TANDEM.pdf>. Listed on
soundiness.org's bibliography.

They instrumented the .NET analyser Clousot so every deliberate unsoundness
became an explicit located assumption, ran it over six open-source projects, and
then checked at runtime how often those assumptions were actually violated.
Eleven categories: invariants at method entries, invariants at call sites,
aliasing, write effects, purity, overflows, conversions, catch blocks, finally
blocks, static fields, main methods.

Results, in their numbers: 978 methods analysed; 326 of them (33%) had zero
assumptions, i.e. were soundly checked; 860 had fewer than five; only 20 had
more than ten. Unsound assumptions were violated during concrete execution in
2–26% of methods depending on the project. And then the finding we should not
skip past: **manual inspection showed no errors were missed because of an
unsound assumption.** Their conclusion is that Clousot's unsoundness "does not
compromise its effectiveness."

That is a negative result for the naive version of our pitch. The residual set
being non-empty does not automatically mean the answer is wrong. It is still
worth showing — the point is that the *user* gets to judge, not the tool — but
we should not claim that unresolved sites are latent bugs. They mostly are not.
Note also the shape of their distribution: a third of methods clean, the rest
with a handful. If our residual counts look like that, the tool is behaving
normally, not badly.

## 2. Analysers that report their own gaps

Maintenance status first, so the rest can be read with the right weight. From
the GitHub API on 2026-09-13 (`repo-status.sh`):

| repo | stars | last push | latest release |
| --- | --- | --- | --- |
| wala/WALA | 869 | 2026-09-10 | v1.8.0 (2026-07-01) |
| soot-oss/soot | 3098 | 2026-09-11 | 4.7.1 (2026-02-23) |
| soot-oss/SootUp | 818 | 2026-09-11 | v3.0.1 (2026-08-07) |
| plast-lab/doop | 222 | 2026-09-10 | no GitHub releases |
| SVF-tools/SVF | 1708 | 2026-08-31 | SVF-3.3 (2026-05-20) |
| facebook/infer | 15706 | 2026-09-10 | v1.3.0 (2026-05-12) |
| cs-au-dk/TAJS | 198 | **2025-02-11** | no releases |
| cs-au-dk/jelly | 437 | 2026-08-12 | v0.13.0 (2026-05-11) |
| vitsalis/PyCG | 364 | **2023-11-26** | no releases |
| SMAT-Lab/Scalpel | 334 | **2024-03-28** | no releases |
| rust-lang/rust-analyzer | 16838 | 2026-09-12 | rolling |
| golang/tools (gopls) | 8000 | 2026-09-11 | gopls/v0.23.0 (2026-07-09) |

TAJS, PyCG and Scalpel are effectively dormant. Scalpel's call graph is a
vendored copy of PyCG (`src/scalpel/call_graph/pycg.py`), so the two Python
entries are one implementation.

### jelly — the best per-site enumeration in a live tool, with a hole in it

Møller's group, `cs-au-dk/jelly`, head `62e60eb4` (2026-08-12). Verified by
reading source and by running it.

Jelly's `AnalysisDiagnostics` counts the residual explicitly:
`totalCallSites`, `callsWithUniqueCallee`, `callsWithMultipleCallees`,
`callsWithNoCallee`, `nativeOnlyCalls`, `externalOnlyCalls`,
`nativeOrExternalCalls`, `functionsWithZeroCallers`,
`unhandledDynamicPropertyReads`, `unhandledDynamicPropertyWrites`. Warnings are
stored as `Map<Node, string>` — keyed on the AST node, so every warning has a
source location — and split into two maps, `warnings` and `warningsUnsupported`,
the second being specifically "unsupported language feature or library
function." A `--zeros` flag prints the located list:

```
Call with zero callees at /…/probe.js:16:1:16:8
```

Measured (`docs/research/probes/20260913-soundiness/jellyprobe/probe.js`, four call shapes: a plain
call, a dispatch table indexed by a computed key, a call through a value from
`require('os')`, and a method on an object produced by `eval`):

```
Calls with zero or one callee: 3/7 (42.86%), multiple: 0/7 (0.00%),
native or external: 4/7 (57.14%)
Warning: Call to 'eval' at /…/probe.js:15:11
Call with zero callees at /…/probe.js:16:1:16:8
```

The `--callgraph-json` output carries `calls` (every call site with its span),
`call2fun` (the edges) and an explicit `ignore` array, so a consumer can derive
the residual as `calls − domain(call2fun)`.

**The hole.** Reduced to the dispatch table alone
(`jellyprobe/probe2.js` — a local object literal with two arrow-function values,
called as `handlers[k]()`):

```
calls:    {'4': '0:2:31:2:44', '5': '0:3:1:3:14'}
call2fun: [[5, 2]]
```

Call 4 has no callee edge. `--zeros` does not print it. It is counted under
"native or external" instead, because the computed key produced an access-path
token. And in the source, the native/external reporters return a count, not a
set — the locations exist but are emitted only at debug log level:

```java
getZeroButExternalCalleeCalls(): number {   // src/output/analysisstatereporter.ts:363
    ...
    if (logger.isDebugEnabled())
        logger.debug(`Call with external-only callees at ${locationToStringWithFile(c.loc)}`);
```

So even the best-engineered residual reporter in a maintained tool partitions
its unresolved calls into four buckets and only enumerates one of them by
default — and the bucket it hides is the one containing dynamic dispatch through
a locally-defined table, which is exactly the shape a refactoring user needs to
see. The union `zero-callee ∪ native-only ∪ external-only ∪ native-or-external`
is the set we would want; jelly counts it and lists a quarter of it.

Jelly's other relevant machinery: `--approx` (approximate interpretation, from
**Møller et al., "Reducing Static Analysis Unsoundness with Approximate
Interpretation,"** [doi:10.1145/3656424](https://doi.org/10.1145/3656424)) tries
to *shrink* the residual, and NodeProf-based dynamic call graph construction
exists to *measure* it. Reduce, measure, partially report.

### TAJS — the reference implementation of the manifesto, and it says so

`cs-au-dk/TAJS`, head `3bdf55a4` (2025-02-11). Dormant, but architecturally the
cleanest thing found.

`src/dk/brics/tajs/analysis/Unsoundness.java` is a single class whose stated job
is: "This class is supposed to contain all implementations of unsound
(micro-)transfers in TAJS. The core analysis implementation should focus on
being sound, and dispatch to this class at locations where unsoundness could be
used." Every unsound choice goes through `addMessage(AbstractNode node, String
msg)`, which records a `Message` against the node's source location with a
dedicated severity, `Severity.TAJS_UNSOUNDNESS` — a distinct enum value
alongside `TAJS_ERROR`, `HIGH`, `MEDIUM`, `LOW`, `TAJS_META`. The flag is
`-show-unsoundness-usage`, "Shows all the usages of unsoundness."

The decision points, each its own toggle and its own message, from the source:
skipping a read of `__proto__` or `constructor`; skipping implicit global
variable declaration; simplifying an imprecise `Function` constructor; ignoring
an imprecise or non-call-node `eval`; assuming `Object.keys` sorted; fixing
`Math.random` and `Date.now`; ignoring a missing model for a native function;
fixing locale; assuming UInt addition closed; skipping prototypes on dynamic
property reads; ignoring imprecise `innerHTML`/`outerHTML` writes; skipping
`__proto__` writes; ignoring an unlikely `undefined` first operand to `+`;
ignoring a value partition; ignoring a potential exception.

There is also `KnownUnsoundnesses.java` — a checked-in catalogue of benchmarks
TAJS is known to be unsound on. **Andreasen, Møller, Nielsen, "Systematic
approaches for increasing soundness and precision of static analyzers," SOAP
2017**, [doi:10.1145/3088515.3088521](https://doi.org/10.1145/3088515.3088521),
PDF at <https://pure.au.dk/ws/files/126130216/10_1.pdf>, says of it: "In TAJS,
we maintain a catalog of known soundness errors, which are then ignored when
running the soundness tests. ... This catalog helps to document the unsoundness
in TAJS, as advocated by the soundiness manifesto." That is the only explicit
manifesto-to-implementation link found.

Two caveats. It is off by default. And it enumerates *unsound transfer
applications*, not unresolved call sites — the two overlap but are not the same
question.

### Doop — an output relation named after the problem

`plast-lab/doop`, `souffle-logic/analyses/sound-may-point-to/analysis.dl`. The
header states the design: "A sound may-point-to analysis. Does not conclude
anything if it is not certain it over-approximates all possible points-to
targets. That is, **an empty points-to set means 'anything can be pointed
to'**." This is **Smaragdakis, Kastrinis, Balatsouras, "Defensive Points-To
Analysis: Effective Soundness via Laziness," ECOOP 2018**,
[doi:10.4230/LIPIcs.ECOOP.2018.23](https://doi.org/10.4230/lipics.ecoop.2018.23).

The residual is a declared output relation:

```
.decl MethodHasUnresolvedInvocation(?ctx: MayContext, ?toMethod:Method)
.output MethodHasUnresolvedInvocation
```

derived from an instruction-granular predicate `InvocationSiteFullyResolved(?ctx,
?invo)` and then propagated transitively through the call graph — so a method
inherits unresolvedness from anything it can reach. There are also
`BasicBlockContainsUnresolvedCall`, `SomePathFromFirstInstructionContainsUnresolvedCall`,
`CalleeHasUnresolvedInvocation`.

Two limits. `InvocationSiteFullyResolved` is *not* declared `.output`, so the
per-instruction fact is computed and discarded; only the method-granular
`MethodHasUnresolvedInvocation` survives to disk. And the whole thing lives in
one analysis variant, not in the default Doop pipeline.

The design principle is the transferable part: make the empty answer *mean*
"unknown", and make the tool never conflate "no targets" with "no targets
found". The paper reports 34–74% program coverage against an unsound
state-of-the-art analysis — i.e. it deliberately answers about a smaller
fraction of the program in exchange for the answers being trustworthy. That is
the trade we are making, with a published precedent and a published cost.

### Soot — the one that turns the residual into a runtime tripwire

`soot-oss/soot`, `soot/jimple/toolkits/callgraph/OnFlyCallGraphBuilder.java`.
This is the most useful idea found and it is nearly twenty years old.

Soot's default reflection handling logs, and only when `-verbose` is set:

```java
logger.warn("Method " + source + " is reachable, and calls Class.newInstance; graph will be incomplete!"
        + " Use safe-newinstance option for a conservative result.");
logger.warn("Call to java.lang.reflect.Method: invoke() from " + container + "; graph will be incomplete!");
logger.warn("InvokeDynamic to " + ie + " not resolved during call-graph construction.");
```

Method-granular, string-formatted, off by default. Unremarkable.

The `TraceBasedReflectionModel` — the TamiFlex integration from **Bodden, Sewe,
Sinschek, Oueslati, Mezini, "Taming Reflection: Aiding Static Analysis in the
Presence of Reflection and Custom Class Loaders," ICSE 2011**,
[doi:10.1145/1985793.1985827](https://doi.org/10.1145/1985793.1985827) — is not.
When the reflection trace has no entry for a reflective call site, Soot builds a
first-class record of it:

```java
static final class Guard {
    final SootMethod container;
    final Stmt stmt;
    final String message;
}
```

and holds a `Set<Guard>` — statement-granular, with a human-readable reason
("Class.forName() call site; Soot did not expect this site to be reached"). Then
`-guards` chooses what to do with the set: `ignore`, `print`, or `throw`. Under
`print` and `throw`, Soot **rewrites the program**, inserting before each
unresolved statement a `java.lang.Error` constructed with that guard's message,
which is either printed with a stack trace or thrown when execution reaches the
site.

That is the answer to "what is the residual set actually for". It is not a
report the user reads and forgets; it is a set of locations you can instrument,
so that the one time it matters, the program tells you. For a refactoring tool
the analogue is obvious and cheap: for each unresolved dispatch point, emit
something the user can paste in — an assertion, a log line, a test — that will
fire if the site is exercised with the old name still live.

Also enumerable in Soot, though far weaker: `Scene.getPhantomClasses()` returns
"a chain of the phantom classes in this scene. These classes are referred to by
other classes, but cannot be [resolved]." Class granularity, in-memory API only,
populated only under `-allow-phantom-refs`, absent from every output artefact.

### WALA — a warning facility that stops just short of call sites

`wala/WALA` at `master`. `com.ibm.wala.core.util.warnings.Warnings` is a global
static `Collection<Warning>` with `add`, `clear`, `iterator` and `asString`.
`Warning` has severity levels `MILD/MODERATE/SEVERE` (plus client variants) and
an abstract `String getMsg()`.

Fourteen `Warning` subclasses exist across the repo. They cover class exclusion,
class-hierarchy lookup failure, entrypoint resolution failure, exception lookup
failure, field resolution failure, checkcast failure, and — from the reflection
interpreter — `NoSubtypesWarning` and `ManySubtypesWarning` for factory methods.
Every `getMsg()` is `getClass() + " : " + <a type or field reference>`. **None
of them carries a call site, a bytecode index, or a source line.**

And unresolved calls specifically are not among them.
`SSAPropagationCallGraphBuilder`, around line 1923, when the receiver type
resolves to no target:

```java
CGNode target = getTargetForCall(node, call.getCallSite(), keys[0].concreteType(), keys);
if (target == null) {
  // This indicates an error; I sure hope getTargetForCall
  // raised a warning about this!
  if (DEBUG) {
    System.err.println("Warning: null target for call " + call);
  }
}
```

The call site is dropped. The comment hopes someone else warned. Nothing in the
`Warning` hierarchy corresponds to it, and the fallback is a `DEBUG`-gated
`System.err`. `handleCall` a few hundred lines earlier does the same thing
silently, returning `false`.

`Warnings.asString()` is called by two example drivers and one test, after class
hierarchy construction, then cleared. It is a developer facility, not an output.

WALA is the clearest case of the pattern we are targeting: the infrastructure to
report the residual exists, is well-designed, and was never wired to the place
where the residual actually appears.

### SVF — has the set, emits the count

`SVF-tools/SVF`. `svf/lib/Util/SVFStat.cpp:209` and
`svf/lib/WPA/AndersenStat.cpp:341-342`:

```cpp
generalNumMap["IndCallSites"] = pag->getIndirectCallsites().size();
PTNumStatMap["IndEdgeSolved"]  = pta->getNumOfResolvedIndCallEdge();
```

Two integers in the statistics block. `getIndirectCallsites()` returns the
actual set and is available in-process, so the enumeration is one loop away, but
nothing in SVF's output prints it. A user learns that 4,000 of 5,000 indirect
call edges were resolved and cannot find out which 1,000 were not without
writing C++.

### Infer — a per-procedure debug summary, not a report

`facebook/infer`, `infer/src/pulse/PulseSkippedCalls.ml`. Pulse's abstract state
carries `skipped_calls: SkippedCalls.t` — a map from `Procname` to a
`PulseTrace`, documented in the record as "metadata: procedure calls for which no
summary was found", with `yojson_of_t` so it serialises. The trace prints as
"call to skipped function occurs here". The same state has
`unknown_values: bool` ("did we generate at least one unknown abstract value on
this path?") and `add_missed_captures`.

Infer's own documentation (`infer/documentation/checkers/Pulse.md`, "Unknown
Functions") is refreshingly direct about the consequences — it shows a worked
false negative and a worked false positive caused by unknown calls — and tells
you how to see them:

```console
$ infer debug --procedures --procedures-filter 'false_negative' --procedures-summary
...
    skipped_calls={ unknown -> call to skipped function occurs here }
```

That is a debug subcommand against one procedure at a time. It is not in
`report.json`, it has no source line in the printed form, and there is no way to
ask "list every skipped call in this run." The state is primarily used
internally to suppress reports, not to inform the user.

### PyCG and Scalpel — the silent case

`vitsalis/PyCG` (last push 2023-11-26). The output format is
`pycg/formats/simple.py`:

```python
output_cg = {}
for node in output:
    output_cg[node] = list(output[node])
return output_cg
```

A dict from caller to callee list. There is no unresolved channel anywhere in
the format — a call PyCG cannot attribute simply produces no entry, and a
consumer cannot tell "this function calls nothing" from "I could not tell what
this calls." Code search for `unresolved` or `unknown` across both repos returns
zero hits. `SMAT-Lab/Scalpel` (last push 2024-03-28) vendors PyCG wholesale at
`src/scalpel/call_graph/pycg.py`, so it inherits the same behaviour.

This is the same failure mode already documented for scip-typescript on
`any`-typed receivers in
`20260913-1345-what-scip-and-the-ts-language-service-actually-resolve.md`:
absence, not a marker.

### SootUp

Code search for `unresolved` in `soot-oss/SootUp` returns five hits, all in the
Jimple text frontend (`JimpleBodyConverterState`, `MethodVisitor`,
`StmtVisitor`, `LazyJimpleMethodSource`) and `ViewTypeHierarchy` — parser-level
forward references, not analysis residuals. No equivalent of Soot's phantom
class chain or guard set was found. Not exhaustively verified; the search was
keyword-based.

## 3. Refactoring tools that surface uncertainty to a user

This is the strongest and most directly applicable prior art, and the least
discussed.

### IntelliJ IDEA — three top-level nodes, and one of them is literally "unresolved"

Verified in `JetBrains/intellij-community` at `master`.

`BaseRefactoringProcessor.createPresentation` partitions every usage into
exactly three buckets before the preview is shown:

```java
if (usage instanceof UsageInfo2UsageAdapter && ((UsageInfo2UsageAdapter)usage).getUsageInfo().isDynamicUsage()) {
  dynamicUsagesCount++;  dynamicUsagesCodeFiles.add(containingFile);
} else if (elementUsage.isNonCodeUsage()) {
  nonCodeUsageCount++;   nonCodeFiles.add(containingFile);
} else {
  codeUsageCount++;      codeFiles.add(containingFile);
}
```

The three node labels come from `UsageViewBundle.properties`:

- `usage.view.results.node.code` — "Code"
- `usage.view.results.node.non.code` — "In Strings, Comments, and Text"
- `usage.view.results.node.dynamic` — "Dynamic" (list form: "Dynamic usages")

Each node carries its own count and file count, expands to a per-file,
per-line tree, and every leaf has an exclusion checkbox. Sub-classification
within non-code uses `usage.type.string.constant` ("Usage in string constants"),
`usage.type.comment` ("Usage in comments"), and `usage.type.unclassified`
("Unclassified").

**The definition of "Dynamic" is the finding.** From
`platform/core-api/.../UsageInfo.java` and
`platform/analysis-api/.../MoveRenameUsageInfo.java`:

```java
myDynamicUsage = reference.resolve() == null;
// and, for poly-variant references:
myDynamicUsage = ((PsiPolyVariantReference)reference).multiResolve(false).length == 0;
```

A "Dynamic usage" in IntelliJ *is* a reference the resolver could not resolve.
JetBrains ships a dedicated, always-present, per-location, individually
excludable category in the rename preview whose entire meaning is "I found this
and I could not confirm it." That is our product proposition, shipped since
2011.

Origin, via the JetBrains YouTrack public API: **IDEA-66364, "Provide a separate
node in rename refactoring preview for dynamic usages"**, filed 2011-03-09 by
`nnmatveev`, resolved 2011-03-11.
<https://youtrack.jetbrains.com/issue/IDEA-66364>. The motivating example in the
comments is PHP:

```php
if (true) { function foo() {} } else { function foo() {} }
foo();
```

— a call whose declaration cannot be determined, so it lands in Dynamic.

Separately, `BaseRefactoringProcessor.processConflicts` takes a
`MultiMap<PsiElement, String>` of conflicts and shows a blocking "conflicts"
dialog *before* the refactoring runs. Conflicts and residuals are distinct
channels: conflicts are things that would break, residuals are things it could
not check.

There is also an explicit incompleteness warning in the bundle, though for a
different cause:
`message.occurrences.in.0.may.be.skipped.load.all.modules.and.repeat.the.search.to.get.complete.results`
— "Occurrences in {0} may be skipped. Load all modules and repeat the search to
get complete results."

**What JetBrains chose to show by default** is itself a data point.
`JavaRefactoringSettings`:

| setting | default |
| --- | --- |
| `RENAME_SEARCH_IN_COMMENTS_FOR_CLASS` / `_METHOD` / `_FIELD` / `_PACKAGE` | `false` |
| `RENAME_SEARCH_IN_COMMENTS_FOR_VARIABLE` | `true` |
| `RENAME_SEARCH_FOR_TEXT_FOR_CLASS` / `_PACKAGE` | `true` |
| `RENAME_SEARCH_FOR_TEXT_FOR_METHOD` / `_FIELD` | `false` |
| `RENAME_SEARCH_FOR_TEXT_FOR_VARIABLE` | `true` |

Comment search is off for the identifier kinds where the name is likely to be a
common word, on for locals. Text search is on for class and package names —
which are distinctive and appear in config files and Spring XML — and off for
methods and fields. A mature vendor's judgment about which residual categories
earn a default. Note that the *Dynamic* node has no such toggle: unresolved
references are always shown.

### Eclipse JDT — has the concept, collapses it to one sentence

Verified in `eclipse-jdt/eclipse.jdt.ui` at `master` (pushed 2026-09-12).

Eclipse's search engine marks each match `A_ACCURATE` or `A_INACCURATE`. A
"potential match" is an occurrence the search found but could not confirm
resolves to the target — the same idea as IntelliJ's Dynamic. But
`RefactoringSearchEngine.groupByCu` reduces the whole set to one boolean:

```java
for (SearchMatch searchMatch : matchList) {
    if (searchMatch.getAccuracy() == SearchMatch.A_INACCURATE)
        hasPotentialMatches = true;
    ...
}
addStatusErrors(status, hasPotentialMatches, hasNonCuMatches);
```

and the user gets one line, from `refactoring.properties`:

> `RefactoringSearchEngine_potential_matches=Found potential matches. Please review changes on the preview page.`

No count, no locations, no category in the preview tree. The user is told to go
look, without being told where. Eclipse's other incompleteness messages are
equally aggregate:

> `RefactoringSearchEngine_binary_match_grouped=Occurrences in binary types in project ''{0}'' have been found. These occurrences will not be updated, which may lead to compile errors if you proceed.`
>
> `ReferencesInBinaryContext_binaryRefsNotUpdated=Binary references to a refactored element have been found. They will not be updated, which may lead to problems if you proceed.`

Eclipse has the per-match accuracy flag and throws it away at presentation time.
That is a cheap win available to us for free: we already know which sites are
uncertain, and the discipline is simply not to collapse them.

### gopls — sound-by-refusal, and one honest admission of silence

`golang/tools`, `gopls/internal/golang/rename.go` and
`gopls/doc/features/transformation.md`.

Go's interface-method problem is real (interfaces are satisfied structurally, so
renaming a concrete method can silently break an implicit satisfaction) and
gopls handles it with the `golang.org/x/tools/refactor/satisfy` package: it
enumerates every conversion, explicit or implicit, from the affected type to an
interface type and checks the renaming preserves it. Renaming initiated *on* an
interface method sets `changeMethods = true` and renames the whole method set.

The presentation choice is the opposite of IntelliJ's. gopls **aborts**:

```go
r.check(obj)
if len(r.conflicts) > 0 {
    // Stop at first error.
    return nil, nil, fmt.Errorf("%s", strings.Join(r.conflicts, "\n"))
}
```

First conflict wins, whole rename fails, error surfaces in the editor. There is
no reviewable residual because there is no partial result. The documented
philosophy: "Renaming should never introduce a compilation error, but it may
introduce dynamic errors."

The remaining unsoundness is documented in prose, exactly as the manifesto
describes:

> "if there is no direct conversion of the affected type to the interface type,
> but there is an intermediate conversion to a broader type (such as `any`)
> followed by a type assertion to the interface type, then gopls may proceed to
> rename the method, causing the type assertion to fail at run time. Similar
> problems may arise with packages that use reflection, such as `encoding/json`
> or `text/template`. There is no substitute for good judgment and testing."

And one line that is worth quoting to anyone who doubts the problem exists. On
renaming a method receiver:

> "Each other receiver that cannot be fully renamed is **quietly skipped**."

Plus, from the file's own TODO list: "renaming a symbol declared in the module
cache (currently proceeds with half of the renaming!)". Also noted there: "make
satisfy work across packages" is still open, so the interface check is
intra-package.

gopls knows about `any`-laundered type assertions and about reflection-driven
frameworks. It has no channel to say "I renamed 612 things and here are the 3
places where an `any` round-trip could bite you." A prose paragraph in the
feature docs is where that information lives.

### rust-analyzer — `dyn Trait` is not the problem; macros are

`rust-lang/rust-analyzer` at `master`.

`dyn Trait` turns out to be a non-issue, and it is worth being clear about why.
Rust has no structural impls: every `impl Trait for Type` is written out. So a
trait method has exactly one name, and renaming it is a nominal-hierarchy
operation. `crates/ide-db/src/search.rs` does exactly that — when the definition
is a trait associated item, it accepts any reference whose classification maps
back to the trait item via `convert_to_def_in_trait`, and widens the search
scope to the trait's scope via `as_trait_assoc_def`. Dynamic dispatch never
needs resolving because both `dyn Trait` and monomorphised calls name the same
trait method. This is the case TypeScript gets wrong and Rust gets right by
language design, not by analysis cleverness.

The hole is macros, and the module documentation of
`crates/ide-db/src/rename.rs` states both the problem and the right answer:

```rust
//! Another can of worms are macros:
//!
//! macro_rules! m { () => { fn f() {} } }
//! m!();
//! fn main() { f() /* <- rename me */ }
//!
//! The correct behavior in such cases is probably to show a dialog to the user.
//! Our current behavior is ¯\_(ツ)_/¯.
```

The maintainers have written down that the right thing is to surface the
uncertainty to the user, and have not done it. Elsewhere the code does refuse
outright — `bail!("Can't rename local that is defined in a macro declaration")`,
`bail!("Cannot rename a non-local definition")` — which is the gopls strategy:
refuse rather than report.

### Does anyone warn "this rename may be incomplete because of dynamic dispatch"?

Not that could be found. IntelliJ's Dynamic node is the closest, and it fires on
unresolved *references*, not on resolved references whose runtime target is
ambiguous — the interface-dispatch case produces a perfectly resolvable
reference and lands in the ordinary Code node. gopls refuses instead of warning.
rust-analyzer refuses instead of warning. Eclipse warns about binary references
and potential matches, not dispatch. A YouTrack search across all JetBrains
projects for rename + reflection/incompleteness warnings returned nothing
relevant.

### Do users act on the non-code and dynamic categories?

**Could not determine.** No published study was found linking IDE refactoring
preview categories to user behaviour, and with web search unavailable this is a
weak negative — absence of a search, not a searched absence. What was checked:
soundiness.org's bibliography (which lists Soares/Gheyi/Massoni, "Automated
behavioral testing of refactoring engines," IEEE TSE 2013 — about finding bugs
*in* refactoring engines, not about how users treat their output); the JetBrains
YouTrack public API; and the source. The only behavioural evidence found is
indirect: JetBrains' shipped defaults above, which are a vendor's revealed
belief about which residual categories are worth a user's attention.

Treat "is a residual list acted upon?" as an open question our own usage data
will have to answer.

## 4. So does the negative result hold?

Partly, and the qualified version is more useful than either extreme.

**False, as stated.** There is prior art, from at least four directions:
research (Christakis et al.'s located explicit assumptions, 2012; conditional
model checking, 2012; defensive points-to, 2018), analysers (TAJS's
`Unsoundness` registry; jelly's `--zeros`; Doop's
`MethodHasUnresolvedInvocation`; Soot's guard set), and IDEs (IntelliJ's Dynamic
node, defined as `reference.resolve() == null`, shipped 2011). Anyone claiming
this is novel will be corrected by the first reviewer who has used IntelliJ.

**True in the specific form that matters.** Nothing found does all four of:

1. per-site, at the same granularity as the answer (Doop is method-granular;
   WALA is type-granular; Soot's default warnings are method-granular; Eclipse
   collapses to a boolean);
2. in the same artefact as the answer, not a debug channel (jelly's external
   bucket, SVF's set, Infer's `skipped_calls`, WALA's `Warnings`, and TAJS's
   whole registry are all behind a flag, a debug level, or an in-process API);
3. covering *all* residual categories rather than one (jelly enumerates
   zero-callee and hides native/external — the bucket that swallowed a plain
   dispatch table in our probe);
4. on by default, so the user sees it without knowing to ask (only IntelliJ's
   Dynamic node qualifies, and it fires on unresolvable references rather than
   unresolvable dispatch).

**And nobody at all** ships the combination we want: a resolved answer set and a
residual set, side by side, in one output, where the residual is specifically
*dispatch points the tool could not attribute* rather than *references it could
not parse*. IntelliJ's Dynamic node catches the reference-level case and lets
the dispatch-level case through as ordinary code. gopls documents the
dispatch-level case in prose and refuses when it can prove a break. That gap is
real and it is ours to fill.

Also worth carrying forward, because it cuts against us: Christakis et al.'s
VMCAI 2015 measurement found no missed errors attributable to Clousot's unsound
assumptions, despite 2–26% of methods having assumptions violated at runtime.
The residual set being non-empty is not evidence that the answer is wrong. Our
claim has to be about *warranted trust*, not about *caught bugs*, and we should
resist the temptation to imply otherwise.

## What to copy

- **IntelliJ's presentation, exactly.** Named top-level categories, each with a
  count and a file count, each expanding to per-location leaves, each leaf
  individually excludable. Not a footnote, not a summary line — a sibling of the
  answer.
- **Soot's guards.** Do not stop at listing the residual; offer to instrument it.
  For each unresolved dispatch point, emit a paste-able assertion or log line
  that fires if that site is reached with the old name live. This is the only
  mechanism found that converts a static residual into evidence.
- **Doop's semantics.** Empty means unknown, never "none found". Make the
  distinction impossible to lose in the data model, not just in the renderer.
- **jelly's partition, without jelly's hole.** Split the residual by cause
  (unresolvable receiver, dynamic key, external boundary, macro/codegen) and
  enumerate *every* bucket. Counting one bucket and listing another is how a
  well-built tool still misleads.

## What not to copy

- Eclipse collapsing a per-match accuracy flag to one boolean and telling the
  user to go look.
- gopls and rust-analyzer refusing outright. Refusal is defensible for an
  automatic edit; it is useless for an audit, which is what we are.
- Everyone's decision to put the residual behind `-verbose`, `--zeros`,
  `-show-unsoundness-usage`, or `infer debug`. The flag is where residual
  reporting goes to die.

## Sources

Papers, all DOIs verified via OpenAlex or Crossref on 2026-09-13:

- Livshits et al., *In Defense of Soundiness: A Manifesto*, CACM 58(2), 2015. [10.1145/2644805](https://doi.org/10.1145/2644805) · pre-print <https://yanniss.github.io/Soundiness-CACM.pdf> · <http://soundiness.org/>
- Christakis, Müller, Wüstholz, *Collaborative Verification and Testing with Explicit Assumptions*, FM 2012. [10.1007/978-3-642-32759-9_13](https://doi.org/10.1007/978-3-642-32759-9_13) · <https://mariachris.github.io/Pubs/FM-2012.pdf>
- Christakis, Müller, Wüstholz, *An Experimental Evaluation of Deliberate Unsoundness in a Static Program Analyzer*, VMCAI 2015. <https://mariachris.github.io/Pubs/VMCAI-2015-TANDEM.pdf> (DOI not separately verified — UNVERIFIED DOI, verified PDF and venue)
- Beyer, Henzinger, Keremoglu, Wendler, *Conditional Model Checking*, FSE 2012. [10.1145/2393596.2393664](https://doi.org/10.1145/2393596.2393664) · <https://arxiv.org/pdf/1109.6926>
- Beyer, Jakobs, *FRed: Conditional Model Checking via Reducers and Folders*, SEFM 2020. [10.1007/978-3-030-58768-0_7](https://doi.org/10.1007/978-3-030-58768-0_7)
- Smaragdakis, Kastrinis, Balatsouras, *Defensive Points-To Analysis: Effective Soundness via Laziness*, ECOOP 2018. [10.4230/LIPIcs.ECOOP.2018.23](https://doi.org/10.4230/lipics.ecoop.2018.23)
- Bodden, Sewe, Sinschek, Oueslati, Mezini, *Taming Reflection*, ICSE 2011. [10.1145/1985793.1985827](https://doi.org/10.1145/1985793.1985827)
- Andreasen, Møller, Nielsen, *Systematic Approaches for Increasing Soundness and Precision of Static Analyzers*, SOAP 2017. [10.1145/3088515.3088521](https://doi.org/10.1145/3088515.3088521) · <https://pure.au.dk/ws/files/126130216/10_1.pdf>
- Møller et al., *Reducing Static Analysis Unsoundness with Approximate Interpretation*, 2024. [10.1145/3656424](https://doi.org/10.1145/3656424) (cited from jelly's README; abstract not read — UNVERIFIED beyond DOI resolution)

Source files read at `master`/`main` on 2026-09-13 (copies in the scratchpad):

- WALA: `core/.../warnings/Warning.java`, `Warnings.java`, `ipa/callgraph/propagation/SSAPropagationCallGraphBuilder.java`, `PropagationCallGraphBuilder.java`, `ipa/cha/ClassHierarchy.java`, `analysis/reflection/AbstractReflectionInterpreter.java`, `examples/drivers/ScopeFileCallGraph.java`
- Soot: `soot/Scene.java`, `soot/jimple/toolkits/callgraph/OnFlyCallGraphBuilder.java` (branch `develop`)
- Doop: `souffle-logic/analyses/sound-may-point-to/analysis.dl`
- SVF: `svf/lib/Util/SVFStat.cpp`, `svf/lib/WPA/AndersenStat.cpp`
- Infer: `infer/src/pulse/PulseSkippedCalls.ml`, `PulseAbductiveDomain.mli`, `infer/documentation/checkers/Pulse.md`
- TAJS: `src/dk/brics/tajs/analysis/Unsoundness.java`, `options/UnsoundnessOptionValues.java`, `solver/Message.java`
- jelly: `src/analysis/diagnostics.ts`, `src/analysis/fragmentstate.ts`, `src/analysis/analyzer.ts`, `src/output/analysisstatereporter.ts`, `src/main.ts`
- PyCG: `pycg/formats/simple.py`; Scalpel: `src/scalpel/call_graph/pycg.py`
- IntelliJ: `platform/usageView/resources/messages/UsageViewBundle.properties`, `platform/usageView/src/.../UsageViewPresentation.java`, `platform/core-api/src/.../UsageInfo.java`, `platform/analysis-api/src/.../MoveRenameUsageInfo.java`, `platform/refactoring/src/.../BaseRefactoringProcessor.java`, `java/java-impl/src/.../JavaRefactoringSettings.java`
- Eclipse JDT: `org.eclipse.jdt.core.manipulation/core extension/.../refactoring/refactoring.properties`, `.../RefactoringSearchEngine.java`
- gopls: `gopls/internal/golang/rename.go`, `gopls/doc/features/transformation.md`
- rust-analyzer: `crates/ide-db/src/rename.rs`, `crates/ide-db/src/search.rs`, `crates/ide/src/rename.rs`

Issue tracker: <https://youtrack.jetbrains.com/issue/IDEA-66364> (fetched via the
YouTrack REST API, saved as `docs/research/probes/20260913-soundiness/yt66364.json`).

## Wrong turns, recorded

- DBLP's JSON API is behind an anti-bot interstitial and returned HTML for both
  `/search/publ/api` and `/pid/*.json`. Semantic Scholar's graph API returned
  HTTP 429 on the first call. OpenAlex (`api.openalex.org/works` with
  `filter=title.search:`) worked for every lookup and is the one to reach for
  first; `oa.sh` in the scratchpad wraps it. Crossref (`api.crossref.org/works/<doi>`)
  is good for confirming authors and venue but returns no abstract for Springer
  LNCS.
- Two mirrors of the FM 2012 PDF (`kar.kent.ac.uk`, `people.inf.ethz.ch`) were
  dead or stubs. The author's own page, `mariachris.github.io/Pubs/`, had every
  paper. Going to the author's homepage first would have saved four fetches.
- Microsoft Research's publication permalinks for pre-2015 papers 404.
- `soundiness.org` serves a certificate for `*.github.com`, so WebFetch refuses
  it; `curl -k` works.
- The GitHub tree API needs `-f recursive=1` rather than a query string in the
  path under `gh api`, and TAJS's `master` tree is only reachable by commit SHA,
  not by branch name.
