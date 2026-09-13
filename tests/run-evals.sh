#!/usr/bin/env bash
# Run the eval suite against a throwaway worktree of the committed tree.
#
# A worktree rather than this directory, for two reasons. The harness refuses a
# plugin containing any file with more than one name, and cargo leaves tens of
# thousands of hard-linked object files under target/; run in place, every case
# fails before an agent starts, with an error about case definitions that points
# nowhere near the cause. And a measurement should not move because someone has
# unsaved edits, so it runs against what is committed.
#
# The built binary is copied in afterwards. Without it the plugin's own preflight
# stops the agent at step zero, correctly, and every case scores nothing for a
# reason that has nothing to do with what is being measured. That was observed
# before this script existed.
#
#   tests/run-evals.sh                        every case
#   tests/run-evals.sh --case trigger-rename  one case
#   tests/run-evals.sh --control              the description deliberately broken
#
# --control answers the question a passing round cannot: whether the round can
# fail at all. It replaces the description with one describing a different tool
# and expects the cases that should fire to stop firing. A round that has never
# failed is not evidence.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
BUILT="$REPO/target/release/resweep"
WORKTREE="$(mktemp -d)/plugin"
CONTROL=0

ARGS=()
for arg in "$@"; do
  if [ "$arg" = "--control" ]; then CONTROL=1; else ARGS+=("$arg"); fi
done

cleanup() {
  git -C "$REPO" worktree remove --force "$WORKTREE" >/dev/null 2>&1
  rm -rf "$(dirname "$WORKTREE")"
}
trap cleanup EXIT

if [ ! -x "$BUILT" ]; then
  if command -v cargo >/dev/null 2>&1; then
    printf 'building the tool first, since the plugin cannot run without it\n'
    (cd "$REPO" && cargo build -p resweep --release --quiet) || {
      printf 'the build failed, so there is nothing to measure\n' >&2; exit 1; }
  else
    printf 'SKIP no build at %s and no cargo to make one\n' "$BUILT"
    exit 0
  fi
fi

if ! git -C "$REPO" worktree add --detach --quiet "$WORKTREE" HEAD; then
  printf 'could not create a worktree; is this a git repository?\n' >&2
  exit 1
fi

mkdir -p "$WORKTREE/target/release"
cp "$BUILT" "$WORKTREE/target/release/resweep"

# The final binary is one file with one name; the hard links are all among
# cargo's intermediate objects, which a worktree does not carry. Checked rather
# than assumed, because the failure it causes points somewhere else entirely.
LINKED="$(find "$WORKTREE" -type f -links +1 | head -3)"
if [ -n "$LINKED" ]; then
  printf 'the worktree holds a file with more than one name:\n%s\n' "$LINKED" >&2
  printf 'the harness will refuse it.\n' >&2
  exit 1
fi

if [ "$CONTROL" -eq 1 ]; then
  python3 - "$WORKTREE/skills/resweep/SKILL.md" <<'BREAK_IT'
import pathlib, sys
p = pathlib.Path(sys.argv[1])
lines = p.read_text().split("\n")
for i, line in enumerate(lines):
    if line.startswith("description:"):
        lines[i] = ("description: Formats YAML configuration files according to a house "
                    "style. Use only when explicitly asked to reformat a YAML file.")
        break
else:
    raise SystemExit("no description line to break")
p.write_text("\n".join(lines))
BREAK_IT
  printf 'control: the description now describes a different tool.\n'
  printf 'control: cases that should fire are expected to STOP firing. A pass here is a failure.\n\n'
fi

cd "$WORKTREE" || exit 1
# Bash because every step of the journey is a shell command: without it the
# with arm fires the skill and then cannot act, which was observed burning
# every remaining turn. --scaffold because a case with no repository in its
# working directory asks a question about code that does not exist, and both
# arms then answer correctly that the directory is empty.
claude plugin eval . \
  --trust-plugin \
  --scaffold \
  --allow-tools Bash \
  --json "$WORKTREE/result.json" \
  "${ARGS[@]}"
STATUS=$?

if [ -f "$WORKTREE/result.json" ]; then
  mkdir -p "$REPO/evals/results"
  cp "$WORKTREE/result.json" "$REPO/evals/results/last-run.json"
  # A target that fails to resolve as a plugin collapses to one arm and reports
  # a single score that reads exactly like a comparison. Assert both ran.
  python3 - "$REPO/evals/results/last-run.json" <<'CHECK_ARMS'
import json, sys
run = json.load(open(sys.argv[1]))
text = json.dumps(run)
arms = set()
for name in ("with", "without"):
    if f'"{name}"' in text:
        arms.add(name)
if arms != {"with", "without"}:
    print(f"\nonly these arms ran: {sorted(arms) or 'none named'}", file=sys.stderr)
    print("a single-arm run reports a score that reads like a comparison and is not one.",
          file=sys.stderr)
    raise SystemExit(1)
print(f"\nboth arms ran, so the delta is a comparison")
CHECK_ARMS
  ARMS=$?
  [ "$ARMS" -eq 0 ] || STATUS=1
  printf 'machine-readable result at evals/results/last-run.json\n'
fi

exit "$STATUS"
