# Sweep: css-colours

**Question.** Is this literal colour a design-system violation that should use a palette custom property, or a case where no token applies?

**Scope.** `apps/web/src`

## Coverage

| | |
|---|---|
| Candidate sites enumerated | 37 |
| Sites judged | 37 |
| Sites unjudged | 0 |
| Verdict `violation` | 29 |
| Verdict `pass` | 8 |
| Verdict `na` | 0 |

> **This sweep did not look at part of its own scope.** It contains 59 `.ts` files, 118 `.tsx` files that no census rule's language can reach, so nothing in those files was ever a candidate and no violation in them can appear below. Add a rule per language and re-census before treating these findings as a picture of the scope.

> Every site the rules reached carries a verdict. That is a complete sweep of the files those rules can parse, not of the scope.

## Census rules

Completeness is relative to these rules. A violation shaped differently from every rule here was never a candidate and does not appear. The rule source is reproduced in full so this claim stays checkable after the rule files themselves have moved or gone.

### `css-literal-colour` (css)

From `../codesweep/rules/css-literal-colour.yml` at census time.

```yaml
# The design system forbids hex codes, rgb()/hsl() functions and the named
# colours white and black in component CSS: every colour must come from a
# palette custom property so that dark mode can override it. The palette file
# itself is the one place literals belong, so a census excludes it by scope
# rather than by rule.
id: css-literal-colour
language: css
rule:
  kind: declaration
  regex: ':\s*(#[0-9a-fA-F]{3,8}\b|(rgb|rgba|hsl|hsla)\(|(white|black)\s*;?\s*$)'
```

Matching is syntactic. ast-grep does no scope, type or dataflow analysis, so it cannot tell one `foo` from another `foo` in a different scope, and a call written in a different shape is a different pattern. A sweep is complete with respect to a syntactic shape, never with respect to a symbol.

## Violations (29)

### `apps/web/src/components/dashboard/AddProjectModal.css:11`

Rule `css-literal-colour`, site `b270910cf479109b`, lines 11-11.

Modal backdrop scrim hardcoded as rgba(0, 0, 0, 0.5). The palette has no scrim token, so
the identical literal appears in six modal files across this scope and cannot be
darkened for dark mode. Fixing it needs a new palette token, not a substitution.

### `apps/web/src/components/dashboard/ExportModal.css:9`

Rule `css-literal-colour`, site `32b1acba4bc805e1`, lines 9-9.

Same hardcoded rgba(0, 0, 0, 0.5) backdrop scrim. Note this file already uses
var(--z-modal-backdrop) on the next line, so the token habit is present and only the
colour was missed.

### `apps/web/src/components/dashboard/RepoBrowserModal.css:9`

Rule `css-literal-colour`, site `0d1418bfbd320536`, lines 9-9.

Same hardcoded rgba(0, 0, 0, 0.5) backdrop scrim, third occurrence of the identical
literal.

### `apps/web/src/components/dashboard/RepoBrowserModal.css:182`

Rule `css-literal-colour`, site `c5f06fd7620eec30`, lines 182-182.

rgba(183, 226, 2, 0.15) is the brand green written out by hand. The palette defines
--ceetrix-green-rgb, so this should be rgba(var(--ceetrix-green-rgb), 0.15).

### `apps/web/src/components/workspace/DeleteTaskWarningModal.css:9`

Rule `css-literal-colour`, site `7dd9fbfc1487e3f4`, lines 9-9.

Same hardcoded rgba(0, 0, 0, 0.5) backdrop scrim, fourth occurrence.

### `apps/web/src/components/workspace/EpicNode.css:171`

Rule `css-literal-colour`, site `1fb83ee358fa2049`, lines 171-171.

rgba(183, 226, 2, 0.1) is the brand green by hand. The file already uses the compliant
rgba(var(--status-error-rgb), ...) form seventeen lines above, so this is an
inconsistency inside a single file.

### `apps/web/src/components/workspace/ForceCompleteDialog.css:10`

Rule `css-literal-colour`, site `909256c4b0e9dc5f`, lines 10-10.

Same hardcoded rgba(0, 0, 0, 0.5) backdrop scrim, fifth occurrence.

### `apps/web/src/components/workspace/ForceCompleteDialog.css:44`

Rule `css-literal-colour`, site `8c435b1ac5aab890`, lines 44-44.

rgba(239, 68, 68, 0.1) is the error red by hand, while the border and colour on the
surrounding lines both use var(--status-error). --status-error-rgb exists and should be
used here.

### `apps/web/src/components/workspace/StoryBacklog.css:32`

Rule `css-literal-colour`, site `c90614ccff964b99`, lines 32-32.

rgba(183, 226, 2, 0.05) is the brand green by hand. Separately, the two lines above
reference var(--accent-primary), which is not defined anywhere in ceetrix-palette.css,
so those two declarations resolve to nothing.

### `apps/web/src/components/workspace/StoryHeader.css:71`

Rule `css-literal-colour`, site `77db303904a2adec`, lines 71-71.

rgba(183, 226, 2, 0.15) brand green by hand on the proposed status badge.

### `apps/web/src/components/workspace/StoryHeader.css:76`

Rule `css-literal-colour`, site `f1d2cea6eac752c8`, lines 76-76.

rgba(183, 226, 2, 0.3) brand green by hand on the in-progress status badge, while the
colour on the next line correctly uses var(--ceetrix-green).

### `apps/web/src/components/workspace/StoryHeader.css:81`

Rule `css-literal-colour`, site `57906c3088e4203f`, lines 81-81.

rgba(74, 222, 128, 0.2) success green by hand. --status-success-rgb is defined and the
next line already uses var(--status-success).

### `apps/web/src/components/workspace/StoryHeader.css:86`

Rule `css-literal-colour`, site `6ab9f837e24192aa`, lines 86-86.

rgba(239, 68, 68, 0.2) error red by hand. --status-error-rgb is defined and the next
line already uses var(--status-error).

### `apps/web/src/components/workspace/StoryHeader.css:91`

Rule `css-literal-colour`, site `106f3af4fc498276`, lines 91-91.

rgba(183, 226, 2, 0.2) brand green by hand on the paused status badge.

### `apps/web/src/components/workspace/StoryHeader.css:96`

Rule `css-literal-colour`, site `d466a830d3e17422`, lines 96-96.

rgba(88, 166, 255, 0.2) is a blue with no palette equivalent. --status-info is defined
but has no -rgb companion, so fixing this needs a palette addition rather than a
substitution.

### `apps/web/src/components/workspace/StoryHeader.css:97`

Rule `css-literal-colour`, site `d15da6b197c690b9`, lines 97-97.

Bare hex #58a6ff on the QA badge text. No dark mode override exists for it, so the QA
badge is the one status badge whose text colour cannot adapt.

### `apps/web/src/components/workspace/StoryHeader.css:167`

Rule `css-literal-colour`, site `62c39b4d5edaffa4`, lines 167-167.

Bare hex #58a6ff repeated in the status dropdown, while the five sibling rules on the
surrounding lines all use palette tokens. QA is the single outlier in a compliant block.

### `apps/web/src/components/workspace/StoryHeader.css:178`

Rule `css-literal-colour`, site `1f7d94970e9485fe`, lines 178-178.

rgba(239, 68, 68, 0.1) error red by hand, with var(--status-error) used for both the
border and the text in the same rule.

### `apps/web/src/components/workspace/StoryTree.css:58`

Rule `css-literal-colour`, site `5077907d28ee62ba`, lines 58-58.

rgba(183, 226, 2, 0.1) brand green by hand on the drag-over highlight. StoryNode.css
expresses the identical effect as rgba(var(--ceetrix-green-rgb), 0.1), so the compliant
form is already in use next door.

### `apps/web/src/components/workspace/StoryTree.css:93`

Rule `css-literal-colour`, site `50e599dd3689ba60`, lines 93-93.

rgba(183, 226, 2, 0.05) brand green by hand. The two lines above also reference
var(--accent-primary), which is undefined in ceetrix-palette.css. This block is a
duplicate of the StoryBacklog create-button rule and carries the same two defects.

### `apps/web/src/components/workspace/TaskEvidenceModal.css:13`

Rule `css-literal-colour`, site `11b35fd187d0a60d`, lines 13-13.

Same hardcoded rgba(0, 0, 0, 0.5) backdrop scrim, sixth and final occurrence in this
scope.

### `apps/web/src/components/workspace/TaskList.css:74`

Rule `css-literal-colour`, site `358ed7d97a7ceb51`, lines 74-74.

rgba(183, 226, 2, 0.15) brand green by hand. This status badge block duplicates
StoryHeader.css line for line, so the two files drift independently.

### `apps/web/src/components/workspace/TaskList.css:79`

Rule `css-literal-colour`, site `96caade22be293f0`, lines 79-79.

rgba(183, 226, 2, 0.3) brand green by hand, duplicating the StoryHeader in-progress
badge.

### `apps/web/src/components/workspace/TaskList.css:84`

Rule `css-literal-colour`, site `83dfa8ef97dcc30d`, lines 84-84.

rgba(74, 222, 128, 0.2) success green by hand while the next line uses var(--status-
success). --status-success-rgb is defined.

### `apps/web/src/components/workspace/TasksEditor.css:412`

Rule `css-literal-colour`, site `7eadcbbeda33c4b1`, lines 412-412.

rgba(255, 255, 255, 0.1) is a hardcoded white overlay for a hover state. In light mode
this brightens an already light surface and is close to invisible, so this is a dark-
mode assumption baked into a shared component.

### `apps/web/src/pages/SignupsAdmin.css:219`

Rule `css-literal-colour`, site `efb1c8656c95759a`, lines 219-219.

Named colour white on a var(--status-success) background. The palette defines --text-on-
brand and --text-on-dark for exactly this, and a named colour cannot be overridden in
dark mode.

### `apps/web/src/pages/SignupsAdmin.css:225`

Rule `css-literal-colour`, site `ae3987d46cfb4da9`, lines 225-225.

Named colour white on a var(--status-error) background. Same fix as the approved badge.

### `apps/web/src/pages/SignupsAdmin.css:255`

Rule `css-literal-colour`, site `c098bc7ba34cba66`, lines 255-255.

Named colour white on an approve button hover. Same fix.

### `apps/web/src/pages/SignupsAdmin.css:265`

Rule `css-literal-colour`, site `f0716b61952da08f`, lines 265-265.

Named colour white on a reject button hover. Same fix. All four white literals sit in
SignupsAdmin.css, which is the only file in the scope using named colours at all.

