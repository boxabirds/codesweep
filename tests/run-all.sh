#!/usr/bin/env bash
# Runs all nine suites and the recorded-output comparison. install.sh is checked against temporary target
# directories; the fixture suite proves the mechanism against hand-counted
# numbers; the surfaces suite proves the claim is relative to a declared list
# of parts; the recheck suite proves a judgement can be caught being wrong;
# the Ceetrix suite proves it all survives a real monorepo; the rename suite
# proves the change of name altered nothing else; the payload suite proves a
# plugin can carry an executable and that the host cannot choose among builds;
# the rust suite proves the port's engine agrees with the one it replaces; the
# eval-harness suite proves a number the measurement produces can be trusted,
# without spending model time to produce one.
# None substitutes for another. The comparison runs last because it is the one
# that says a change nobody intended has happened, and it is easier to read
# after the suites that say what does work.
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
STATUS=0
for suite in "$HERE/test_install.sh" "$HERE/test_resweep.sh" "$HERE/test_surfaces.sh" "$HERE/test_recheck.sh" "$HERE/test_against_ceetrix.sh" "$HERE/test_rename.sh" "$HERE/test_plugin_payload.sh" "$HERE/test_rust.sh" "$HERE/test_eval_harness.sh" "$HERE/capture-reference.sh"; do
  printf '\n=== %s ===\n' "$(basename "$suite")"
  # A suite that has been renamed or deleted must fail loudly. Left to bash it
  # produces a command-not-found line among a hundred passing checks and the
  # run still looks healthy, which is the exact failure this tool exists to
  # remove, in the tool's own test harness.
  if [ ! -x "$suite" ]; then
    printf 'MISSING: %s is not present or not executable\n' "$suite"
    STATUS=1
    continue
  fi
  "$suite" || STATUS=1
done
exit "$STATUS"
