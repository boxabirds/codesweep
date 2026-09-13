#!/usr/bin/env bash
# Run the eval cases against a staged copy of this plugin.
#
# Staged rather than run in place, for a reason that is not cosmetic: the
# harness refuses a plugin containing any file with more than one name, and
# cargo's build output under target/ holds tens of thousands of hard-linked
# object files. Run in place, every case fails before an agent starts, with an
# error about case definitions that points nowhere near the cause.
#
# The copy is of the working tree, not of the last commit, so an uncommitted
# change to the skill is what gets measured. Build output and previous results
# are left behind; nothing else is.
#
#   evals/run.sh                          every case
#   evals/run.sh --case trigger-rename    one case
#   evals/run.sh --control                the description deliberately broken
#
# --control answers the question a passing round cannot: whether the round can
# fail at all. It replaces the description with one that describes a different
# tool and expects the audit case to stop firing. A round that has never failed
# is not evidence.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
STAGE="$(mktemp -d)/plugin"
CONTROL=0

ARGS=()
for arg in "$@"; do
  if [ "$arg" = "--control" ]; then CONTROL=1; else ARGS+=("$arg"); fi
done

cleanup() { rm -rf "$(dirname "$STAGE")"; }
trap cleanup EXIT

mkdir -p "$STAGE"
# Copied with tar rather than cp so the exclusions are one list rather than a
# find expression, and so nothing follows a symlink out of the repository.
(cd "$REPO" && tar --exclude=./target --exclude=./.git --exclude=./evals/results -cf - .) \
  | tar -xf - -C "$STAGE"

REMAINING="$(find "$STAGE" -type f -links +1 | head -3)"
if [ -n "$REMAINING" ]; then
  printf 'the staged copy still holds a file with more than one name:\n%s\n' "$REMAINING" >&2
  printf 'the harness will refuse it. Add the directory to the exclusions above.\n' >&2
  exit 1
fi

if [ "$CONTROL" -eq 1 ]; then
  python3 - "$STAGE/skills/resweep/SKILL.md" <<'BREAK_IT'
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
  printf 'control: the description has been replaced with one describing a different tool.\n'
  printf 'control: the audit case is expected to STOP firing. A pass here is a failure.\n\n'
fi

cd "$STAGE" || exit 1
claude plugin eval . --trust-plugin --scaffold --allow-tools Bash --ablation none "${ARGS[@]}"
STATUS=$?

# The report lands inside the staged copy, which is about to go.
RESULTS="$STAGE/evals/results"
if [ -d "$RESULTS" ]; then
  mkdir -p "$HERE/results"
  cp -R "$RESULTS"/* "$HERE/results/" 2>/dev/null
  printf '\nreports copied to evals/results\n'
fi
exit "$STATUS"
