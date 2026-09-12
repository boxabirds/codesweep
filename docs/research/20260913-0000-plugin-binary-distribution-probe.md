# Can a plugin carry and run a machine-specific build?

Measured on 13 September 2026, macOS 25.4, arm64, cargo 1.94.0, before any of the
port in story 8 was written. Task 8.1 exists to answer this first, because two of
the three distribution shapes in the design depend on the answer.

## The three questions, answered

**Does the executable bit survive installation? Yes.**

A trivial Rust binary was built, placed at `bin/` in a plugin, published through a
local marketplace and installed with `claude plugin install`. The installed copy
is mode `-rwxr-xr-x` and its sha256 is identical to the build. It runs.

**Can it be invoked by the plugin-relative path? Yes.**

The installed plugin root is
`~/.claude/plugins/cache/<marketplace>/<plugin>/<version>/`, and `bin/<name>`
under it executes normally. `${CLAUDE_PLUGIN_ROOT}` resolves to that directory.

**Is there any mechanism by which the host selects among several builds? No.**

Neither published schema has any field for it. The plugin manifest offers
`agents, author, channels, commands, dependencies, description, homepage, hooks,
keywords, license, lspServers, mcpServers, monitors, name, outputStyles,
repository, settings, skills, themes, userConfig, version`. The marketplace entry
adds `category, source, strict, tags`. There is no `os`, `cpu`, `arch`,
`platform`, `binary`, `install` or `postinstall` anywhere in either, and no
post-install script hook.

Five source types exist: a path relative to the marketplace root, npm, a git URL,
a GitHub repo, and a git URL with a subpath. Four of those five fetch one tree and
extract it. Only npm brings a selection mechanism of its own.

## What npm would give, and what is still unmeasured

npm's `os` and `cpu` fields combined with `optionalDependencies` are the standard
way a Rust binary reaches many platforms; `@ast-grep/cli` is distributed exactly
this way. Verified directly: a host package with two platform packages as optional
dependencies, one declared `darwin/arm64` and one `linux/x64`, installs only the
matching one on this machine, and the binary inside it runs.

Not verified: whether that resolution happens when the host installs an
npm-sourced plugin. `claude plugin install` appends `@latest` to the package spec,
which a local tarball path cannot satisfy, so the route could not be exercised
without publishing to a registry. Settling it needs a package on a registry with
per-platform optional dependencies and a plugin manifest. Until then the npm route
is a possibility, not a measured capability.

It also carries a cost that should be stated rather than discovered: it requires
npm on the machine, and it is npm doing the selecting rather than the host.

## What a build actually weighs

A binary linking `ast-grep-core`, `ast-grep-config`, `ast-grep-language` 0.45.3
and `rusqlite` with bundled SQLite, built at `opt-level = "z"` with fat LTO, one
codegen unit, stripped and `panic = "abort"`:

| grammars linked | size |
| --- | --- |
| all 27 (`builtin-parser`, the default) | 42.4 MB |
| four (`napi-lang`: css, html, javascript, typescript) | 4.9 MB |

The grammars are almost the whole binary. A first build measured 1.15 MB, which
was wrong in an instructive way: nothing in it actually parsed, so the linker had
dropped every grammar. The 42.4 MB figure is from a build that parses TypeScript
and counts two `catch_clause` matches, so the grammar is genuinely present.

Five platform targets at 42 MB each is over 200 MB in a repository that is cloned
on every install. Carrying every build in the plugin is not viable.

## Trimming the grammars is not free

With a language's feature off, `SupportLang::from_str("python")` still succeeds
and a Python rule file still parses. The failure arrives at parse time, as a
panic:

```
not implemented: tree-sitter parser is not implemented when feature flag is off.
```

So a trimmed build advertises languages it cannot parse and then crashes on them.
Any subsetting has to gate on the tool's own list of linked languages before
handing a rule to ast-grep. This is the same class of defect as the wrong-grammar
problem: the tool must know what it cannot do and say so, rather than proceeding.

## The precedent in Anthropic's own plugins

`rust-analyzer-lsp`, `typescript-lsp` and `pyright-lsp` are official plugins whose
whole purpose is a machine-specific executable. None of them carries one. Each is
a manifest and a README telling the operator to install the server with rustup,
Homebrew, apt or a release download. Where the first-party plugins had this exact
problem, they chose not to ship the binary.

## Consequence for the design

The plugin is not the delivery channel for the build. Three shapes remain:

1. The operator installs the binary and the plugin points at it. This is what the
   first-party plugins do, and it needs no mechanism that does not already exist.
2. The plugin carries a small launcher that locates a build already on the machine
   or fetches the right one on first use. This keeps a single install gesture at
   the cost of a network fetch the operator did not ask for.
3. The binary is published to npm with per-platform optional dependencies and the
   plugin depends on it. This is the only route that gets machine selection for
   free, and it is the one route not yet verified end to end through the host.

The port itself is unaffected: all three consume the same binary. What changes is
what `install.sh` and the skill guidance say, which is task 8.7.
