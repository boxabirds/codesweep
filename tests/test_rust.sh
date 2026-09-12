#!/usr/bin/env bash
# The Rust port's own tests, run alongside everything else.
#
# Kept as a wrapper rather than left to whoever remembers to type cargo test,
# because a suite that is not in the one command nobody runs it. Only the port
# crate is built: the vendored scanner crate is on its way out and its tests
# are not this story's.
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"

if ! command -v cargo >/dev/null 2>&1; then
  printf '  SKIP cargo is not installed, so the port cannot be built or tested\n'
  printf '\n0 passed, 0 failed, 1 skipped\n'
  exit 0
fi

OUT="$(cd "$REPO" && cargo test -p resweep 2>&1)"
STATUS=$?
printf '%s\n' "$OUT" | grep -E '^(test|error|warning: unused|failures:)' | grep -v '^test result' | sed 's/^/  /'

# Summed across the unit, integration and doc test binaries, so a suite that
# silently stopped running shows up as a smaller number rather than as nothing.
PASS="$(printf '%s\n' "$OUT" | grep -oE '^test result: ok\. [0-9]+' | grep -oE '[0-9]+' | paste -sd+ - | bc)"
FAIL="$(printf '%s\n' "$OUT" | grep -oE '[0-9]+ failed' | grep -oE '[0-9]+' | paste -sd+ - | bc)"
printf '\n%s passed, %s failed\n' "${PASS:-0}" "${FAIL:-0}"
exit "$STATUS"
