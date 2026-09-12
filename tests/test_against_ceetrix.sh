#!/usr/bin/env bash
# Integration test against a real, large, messy codebase rather than a fixture.
#
# Fixtures prove the mechanism. They cannot prove the tool survives a monorepo
# with mixed languages, generated files, vendored bundles and a .gitignore that
# matters. Ceetrix is that codebase.
#
# The assertions here are deliberately NOT frozen counts. Ceetrix changes daily,
# so an exact number would fail for the wrong reason within a day and get
# deleted. Instead every assertion is either an internal invariant that holds
# whatever the repository contains, or a cross-check against ripgrep as an
# independent oracle. That is the same discipline the skill demands of a census
# rule: prove the count a second way.
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
CODESWEEP="$HERE/../bin/codesweep"
RULES="$HERE/../rules"
REPO="${CEETRIX_REPO:-$HOME/expts/claude-backlog}"
SCOPE="workers/admin/src"

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

jqp() { python3 -c "import json,sys; d=json.load(sys.stdin); print($1)"; }

if [ ! -d "$REPO/$SCOPE" ]; then
  echo "SKIP: no Ceetrix checkout at $REPO"
  echo "      set CEETRIX_REPO to point at one, or ignore this suite."
  exit 0
fi
command -v rg >/dev/null 2>&1 || { echo "SKIP: ripgrep is needed as the independent oracle"; exit 0; }

export CODESWEEP_SESSION_ID="ceetrix-test-$$-$(date +%s)"
SESSION_DIR="$(python3 -c 'import os,tempfile; print(os.path.join(tempfile.gettempdir(), "codesweep", os.environ["CODESWEEP_SESSION_ID"]))')"
trap 'rm -rf "$SESSION_DIR"' EXIT

echo "sweeping $REPO/$SCOPE"

# --- the combined sweep, both languages ------------------------------------

BOTH=$("$CODESWEEP" --root "$REPO" census both \
  --rule "$RULES/ts-catch-clause.yml" \
  --rule "$RULES/tsx-catch-clause.yml" \
  --scope "$SCOPE" \
  --question "Does this catch clause swallow a failure the caller needed to see?")
BOTH_N=$(echo "$BOTH" | jqp 'd["sites_found"]')

echo "the sweep finds something at all"
check "combined census is non-empty" "True" "$([ "$BOTH_N" -gt 0 ] && echo True || echo False)"

echo "ripgrep agrees, as an independent oracle"
# rg counts lines carrying a catch token. Every real catch clause has one, so rg
# must be at least the AST count. It can exceed it, because rg also matches the
# token inside comments and strings, which the parser correctly ignores.
RG_N=$(rg --no-heading -c -g '*.ts' -g '*.tsx' 'catch\s*[({]' "$REPO/$SCOPE" 2>/dev/null \
  | awk -F: '{s+=$2} END {print s+0}')
check "ripgrep count is at least the AST count" "True" \
  "$([ "$RG_N" -ge "$BOTH_N" ] && echo True || echo False)"
printf '       ast=%s rg=%s delta=%s (delta is comments and strings rg cannot exclude)\n' \
  "$BOTH_N" "$RG_N" "$((RG_N - BOTH_N))"

# --- the tsx language trap -------------------------------------------------

echo "a typescript-only rule silently misses .tsx, and says so"
TS_ONLY=$("$CODESWEEP" --root "$REPO" census tsonly \
  --rule "$RULES/ts-catch-clause.yml" --scope "$SCOPE" \
  --question "Does this catch clause swallow a failure the caller needed to see?")
TS_N=$(echo "$TS_ONLY" | jqp 'd["sites_found"]')
check "typescript-only finds fewer" "True" "$([ "$TS_N" -lt "$BOTH_N" ] && echo True || echo False)"
check "and warns about .tsx" "True" \
  "$(echo "$TS_ONLY" | jqp "'.tsx' in d.get('WARNING_uncovered_extensions', {})")"

TSX_ONLY=$("$CODESWEEP" --root "$REPO" census tsxonly \
  --rule "$RULES/tsx-catch-clause.yml" --scope "$SCOPE" \
  --question "Does this catch clause swallow a failure the caller needed to see?")
TSX_N=$(echo "$TSX_ONLY" | jqp 'd["sites_found"]')

echo "the two languages partition the combined set exactly"
# This invariant holds whatever Ceetrix contains, which is why it is worth
# asserting: the languages are disjoint by file extension, so their counts must
# sum. If they ever do not, a file is being double-counted or dropped.
check "ts + tsx = both" "$BOTH_N" "$((TS_N + TSX_N))"

# --- determinism -----------------------------------------------------------

echo "a repeated census over unchanged code is a no-op"
AGAIN=$("$CODESWEEP" --root "$REPO" census both \
  --rule "$RULES/ts-catch-clause.yml" --rule "$RULES/tsx-catch-clause.yml" \
  --scope "$SCOPE")
check "no new sites"      0 "$(echo "$AGAIN" | jqp 'd["sites_new"]')"
check "no departed sites" 0 "$(echo "$AGAIN" | jqp 'd["sites_departed"]')"
check "same total"        "$BOTH_N" "$(echo "$AGAIN" | jqp 'd["sites_found"]')"

# --- the manifest ----------------------------------------------------------

echo "the manifest is the complete citable record"
MAN=$("$CODESWEEP" --root "$REPO" manifest both)
check "manifest rows equal census count" "$BOTH_N" \
  "$(printf '%s\n' "$MAN" | grep -cE '^  [0-9a-f]{16}  ')"
check "every row has a distinct id" "$BOTH_N" \
  "$(printf '%s\n' "$MAN" | grep -oE '^  [0-9a-f]{16}' | sort -u | wc -l | tr -d ' ')"
check "manifest names both rules" "True" \
  "$(printf '%s' "$MAN" | grep -q 'ts-catch-clause' && printf '%s' "$MAN" | grep -q 'tsx-catch-clause' && echo True || echo False)"

echo "the manifest stays small enough to keep in context"
MAN_BYTES=$(printf '%s' "$MAN" | wc -c | tr -d ' ')
PER_SITE=$((MAN_BYTES / BOTH_N))
check "under 120 bytes per site" "True" "$([ "$PER_SITE" -lt 120 ] && echo True || echo False)"
printf '       %s bytes for %s sites, %s per site\n' "$MAN_BYTES" "$BOTH_N" "$PER_SITE"

echo "every manifest row points at a real line of the file it names"
BAD=$(printf '%s\n' "$MAN" | REPO="$REPO" python3 -c "
import sys, os
repo = os.environ['REPO']
current = None
bad = 0
for line in sys.stdin:
    line = line.rstrip('\n')
    if line.startswith('  ') and current:
        parts = line.split()
        start = int(parts[1].split('-')[0])
        try:
            with open(os.path.join(repo, current), errors='replace') as fh:
                if start > len(fh.readlines()):
                    bad += 1
        except OSError:
            bad += 1
    elif line and not line.startswith(' ') and '/' in line:
        current = line
print(bad)
" 2>/dev/null || echo "err")
check "rows pointing past end of file" 0 "$BAD"

# --- the repository is left alone ------------------------------------------

echo "nothing is written into the repository being swept"
check "no .codesweep directory created" "False" \
  "$([ -e "$REPO/.codesweep" ] && echo True || echo False)"
check "index is in the session temp dir" "True" \
  "$(ls "$SESSION_DIR"/*.db >/dev/null 2>&1 && echo True || echo False)"

echo "the sweep leaves the working tree untouched"
if git -C "$REPO" rev-parse --git-dir >/dev/null 2>&1; then
  check "no modified tracked files from the sweep" "True" \
    "$(git -C "$REPO" diff --name-only -- "$SCOPE" | grep -q . && echo False || echo True)"
fi

# --- a second language, to prove the guard is not tsx-specific -------------

echo "a css rule over a mixed directory reports what it could not reach"
CSS=$("$CODESWEEP" --root "$REPO" census css \
  --rule "$RULES/css-literal-colour.yml" --scope apps/web/src \
  --question "Is this literal colour a design-system violation?" 2>/dev/null)
if [ -n "$CSS" ]; then
  check "css sweep warns about unreached source" "True" \
    "$(echo "$CSS" | jqp "bool(d.get('WARNING_uncovered_extensions'))")"
  check "and status refuses to call it complete" "False" \
    "$("$CODESWEEP" --root "$REPO" status css | jqp 'd["complete"]')"
fi

printf '\n%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
