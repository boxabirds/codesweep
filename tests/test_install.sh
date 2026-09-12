#!/usr/bin/env bash
# Tests for install.sh. Every case runs against temporary target directories via
# RESWEEP_BIN_DIR and RESWEEP_SKILL_DIR, so nothing here touches the real
# ~/.claude or anything on PATH.
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
INSTALL="$HERE/../install.sh"
PASS=0
FAIL=0

check() {
  local label="$1" expected="$2" actual="$3"
  if [ "$expected" = "$actual" ]; then
    printf '  ok   %s (%s)\n' "$label" "$actual"
    PASS=$((PASS + 1))
  else
    printf '  FAIL %s: expected %s, got %s\n' "$label" "$expected" "$actual"
    FAIL=$((FAIL + 1))
  fi
}

TEMP_ROOT="$(cd "$(dirname "$(mktemp -u)")" && pwd)"
WORK="$(mktemp -d)"

cleanup() {
  # Guarded rather than a bare recursive delete: WORK is only ever removed when
  # it is genuinely under the system temp root. An empty or unexpected value
  # refuses instead of deleting something else.
  case "$WORK" in
    "$TEMP_ROOT"/*) rm -rf "$WORK" ;;
    *) printf 'refusing to remove %s: not under %s\n' "$WORK" "$TEMP_ROOT" >&2 ;;
  esac
}
trap cleanup EXIT

run() {
  # Run install.sh against this test's target directories, never the real ones.
  RESWEEP_BIN_DIR="$1/bin" RESWEEP_SKILL_DIR="$1/skills" "$INSTALL" "${2:-}" 2>&1
}

# --- a first install creates both links -------------------------------------

echo "a first install creates both links"
A="$WORK/first"
OUT="$(run "$A")"
check "cli linked"            "True" "$([ -L "$A/bin/resweep" ] && echo True || echo False)"
check "skill linked"          "True" "$([ -L "$A/skills/resweep" ] && echo True || echo False)"
check "skill is readable"     "True" "$([ -f "$A/skills/resweep/SKILL.md" ] && echo True || echo False)"
check "cli link is executable" "True" "$([ -x "$A/bin/resweep" ] && echo True || echo False)"
check "reports two changes"   2 "$(printf '%s\n' "$OUT" | grep -c 'done')"

# --- re-running changes nothing ---------------------------------------------

echo "re-running is idempotent"
BEFORE_CLI="$(readlink "$A/bin/resweep")"
BEFORE_SKILL="$(readlink "$A/skills/resweep")"
OUT="$(run "$A")"
check "no further changes"     0 "$(printf '%s\n' "$OUT" | grep -c 'done')"
check "says nothing to do" "True" "$(printf '%s' "$OUT" | grep -q 'already installed' && echo True || echo False)"
check "cli link unchanged"   "$BEFORE_CLI"   "$(readlink "$A/bin/resweep")"
check "skill link unchanged" "$BEFORE_SKILL" "$(readlink "$A/skills/resweep")"

# --- a link pointing elsewhere is repointed ---------------------------------

echo "a link pointing somewhere else is repointed, and says so"
ln -sfn /dev/null "$A/bin/resweep"
OUT="$(run "$A")"
check "repointed"          "True" "$(printf '%s' "$OUT" | grep -q 'repointed' && echo True || echo False)"
check "names the old target" "True" "$(printf '%s' "$OUT" | grep -q '/dev/null' && echo True || echo False)"
check "now points at source" "$BEFORE_CLI" "$(readlink "$A/bin/resweep")"

# --- a real file is never clobbered -----------------------------------------

echo "a real file where a link should go is refused, not overwritten"
B="$WORK/occupied"
mkdir -p "$B/bin"
printf 'a file that is not ours\n' > "$B/bin/resweep"
OUT="$(run "$B")"
RC=$?
check "exits non-zero"        1 "$RC"
check "says not a symlink" "True" "$(printf '%s' "$OUT" | grep -q 'not a symlink' && echo True || echo False)"
check "file left intact" "a file that is not ours" "$(cat "$B/bin/resweep")"

# --- check mode changes nothing ---------------------------------------------

echo "--check reports state without changing it"
C="$WORK/checkonly"
OUT="$(run "$C" --check)"
check "no links created"  "False" "$([ -e "$C/bin/resweep" ] && echo True || echo False)"
check "reports missing"    "True" "$(printf '%s' "$OUT" | grep -q 'not linked' && echo True || echo False)"
check "no changes claimed"     0 "$(printf '%s\n' "$OUT" | grep -c 'done')"

# --- uninstall removes its own links and nothing else -----------------------

echo "--uninstall removes only the links it created"
OUT="$(run "$A" --uninstall)"
check "cli link gone"   "False" "$([ -L "$A/bin/resweep" ] && echo True || echo False)"
check "skill link gone" "False" "$([ -L "$A/skills/resweep" ] && echo True || echo False)"
check "reports two removals" 2 "$(printf '%s\n' "$OUT" | grep -c 'removed:')"

echo "--uninstall leaves a foreign file alone"
OUT="$(run "$B" --uninstall)"
check "foreign file survives" "a file that is not ours" "$(cat "$B/bin/resweep")"
check "says it left it alone" "True" "$(printf '%s' "$OUT" | grep -q 'leaving it alone' && echo True || echo False)"

echo "--uninstall twice is a no-op"
OUT="$(run "$A" --uninstall)"
check "nothing removed"     0 "$(printf '%s\n' "$OUT" | grep -c 'removed:')"
check "reports absent"  "True" "$(printf '%s' "$OUT" | grep -q 'already absent' && echo True || echo False)"

# --- the source tree is never modified --------------------------------------

echo "installing does not modify the repository"
if git -C "$HERE/.." rev-parse --git-dir >/dev/null 2>&1; then
  # Compared before and after rather than against a clean tree. Checking the
  # absolute state made this fail whenever the developer running the suite had
  # uncommitted work in these paths, which is most of the time while the tool
  # is being changed, and a check that cries wolf during development is one
  # people learn to ignore.
  BEFORE="$(git -C "$HERE/.." diff --name-only -- bin skills install.sh)"
  run "$WORK/clean" >/dev/null
  AFTER="$(git -C "$HERE/.." diff --name-only -- bin skills install.sh)"
  check "no tracked file modified" "$BEFORE" "$AFTER"
fi

# --- an unknown option is rejected ------------------------------------------

echo "an unknown option is rejected"
"$INSTALL" --nonsense >/dev/null 2>&1
check "exit code" 2 "$?"

printf '\n%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
