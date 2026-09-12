#!/usr/bin/env bash
# Runs all three suites. install.sh is checked against temporary target
# directories; the fixture suite proves the mechanism against hand-counted
# numbers; the Ceetrix suite proves it survives a real monorepo. None
# substitutes for another.
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
STATUS=0
for suite in "$HERE/test_install.sh" "$HERE/test_codesweep.sh" "$HERE/test_against_ceetrix.sh"; do
  printf '\n=== %s ===\n' "$(basename "$suite")"
  "$suite" || STATUS=1
done
exit "$STATUS"
