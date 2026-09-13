# What nine languages actually return

2026-09-13. Measured, not reasoned. Every fixture states its real call count
before the tool runs on it, so each number below is a difference between two
known quantities rather than a description of tool output.

Languages chosen in the order codebases are written in: TypeScript, tsx,
JavaScript, jsx and Python first; then PHP, Rust and Go; then C# and the
ASP.NET page kinds. Earlier probing used C and Go, which was drama rather than
relevance.

Each fixture holds the abstractions that actually defeat shape matching —
inheritance, prototypes, aliases, member calls, function values, macros,
dynamic lookup — plus the same call written in a comment and in a string, so
every fixture doubles as evidence about the thing people assume is a problem.

Rule in every case: `pattern: audit($$$ARGS)` for the language under test.

## Tier 1: TypeScript, tsx, JavaScript, jsx, Python

| Language | Reachable | Found | Missed |
|---|---|---|---|
| typescript | 4 | 3 | `const fn = audit; fn('x')` |
| tsx | 2 | 2 | nothing |
| javascript | 4 (2 in .js by name, 1 member, 1 in .jsx) | 3 | `m.audit('member')` |
| python | 4 | 2 | the alias, and `getattr(__import__('log'), 'audit')('x')` |

Found in every position tried: inside a class method, inside an object
literal, inside a prototype assignment, inside tsx markup, inside jsx markup.

The javascript rule reached both `.js` and `.jsx`, confirming there is no
jsx/javascript split. The member call is a limit of the rule the operator
wrote, not of the tool — a second rule finds it.

## Tier 2: PHP, Rust, Go

| Language | Reachable | Found | Missed |
|---|---|---|---|
| php | 4 | 2 | `$fn = 'audit'; $fn(...)` and `call_user_func('audit', ...)` |
| rust | 3 | 2 | the call inside `println!` |
| go | 2 by name | 2 | interface dispatch, and `via(audit)` (correctly, it is a call to `via`) |

Rust is the severe one and has no counterpart in tier 1. In the same file:

    kind: call_expression   -> 4
    kind: macro_invocation  -> 1

The call inside `println!` is not a call_expression at all. The macro body is
an unparsed token tree, so no rule of any shape reaches inside it. Rust code
puts calls inside `println!`, `assert!`, `format!` and `vec!` constantly.

Note the direction against C, measured earlier: C over-counts, because a macro
invocation looks like a call. Rust under-counts, because a call inside a macro
looks like nothing. Mirror images.

Go declares no relationship between a type and an interface it satisfies, so
implementations of an interface are not findable by shape at all.

## Tier 3: C#, aspx

C#: 3 reachable, 2 found, the delegate missed. Behaves like tier 1.

`.aspx` is not handled at all, and the way it is not handled is a separate
defect — see below.

## What is NOT a limit, and was checked

**Comments and strings.** Every language above: the call written once for
real, once in a comment and once in a string literal yields exactly one match.
The parser distinguishes them and always has. This is the most commonly
assumed limit and it does not exist. Recording it would train operators to
distrust numbers that are right.

**jsx.** There is no jsx grammar to name; `language: jsx` is refused. One rule
naming `javascript` reaches `.js` and `.jsx` alike, with zero ERROR nodes on
markup. The tool's own census warning claimed the opposite until 21fa13f.

## A separate defect, found while checking .aspx

A file whose extension is not in `LANGUAGE_EXTENSIONS` is not counted as
source at all, so it is neither examined nor reported as uncovered.

Measured. Eleven files: one `.js` with one call, and ten of
`.aspx .cshtml .razor .vue .svelte .erb .hbs .twig .mdx .astro`.

    census -> sites_found: 1, and no warning of any kind
    status -> uncovered_extensions: {}, uncovered: "{}"

Ten of eleven files were never opened, and the tool positively stated that
nothing had gone uncovered. Not an omission — a false claim.

Cause is one line, `crates/resweep/src/commands/census.rs:32`:

    if languages::SOURCE_EXTENSIONS.contains(&ext.as_str()) {

`SOURCE_EXTENSIONS` is derived from `LANGUAGE_EXTENSIONS`, so "is this source
we failed to read" is answered by "is this something we can read", which can
only come back no. Every unknown format is invisible by construction.

The module's doc comment argues the opposite case for scss, correctly, and the
argument was applied to one extension out of every extension in the world.
Third instance of this class: scss, then jsx, now this. Story 16.

## What this feeds

- Story 15's register is reseeded from the table above, tier 1 first.
- Story 16 exists because of the last section.
- The fixtures live in the story tasks and become the register's
  demonstrations, kept as they are rather than tidied.
