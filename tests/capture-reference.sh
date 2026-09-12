#!/usr/bin/env bash
# Records what every command prints, or compares against what was recorded.
#
# Used across a change that is supposed to alter nothing, so that a difference
# discovered afterwards cannot be mistaken for an improvement. Run with
# `record` before the change and `compare` after.
#
#   tests/capture-reference.sh record
#   tests/capture-reference.sh compare
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
MODE="${1:-compare}"
OUT="$HERE/reference"
TOOL="${RESWEEP_BIN:-$HERE/../bin/resweep}"

export RESWEEP_SESSION_ID="reference-$$-$(date +%s)"
SESSION_DIR="$(python3 -c 'import os,tempfile; print(os.path.join(tempfile.gettempdir(), "resweep", os.environ["RESWEEP_SESSION_ID"]))')"
WORK="$(mktemp -d)"
RULES="$(mktemp -d)"
cleanup() { rm -rf "$WORK" "$RULES" "$SESSION_DIR"; }
trap cleanup EXIT

cat > "$RULES/catch.yml" <<'EOF'
id: ts-catch
language: typescript
rule:
  kind: catch_clause
EOF

mkdir -p "$WORK/src"
cd "$WORK" && git init -q .
printf 'export function a() { try { x(); } catch (e) { return []; } }\n' > src/a.ts
printf 'export function b() { try { x(); } catch (e) { throw e; } }\n' > src/b.ts
git add -A >/dev/null 2>&1
git -c user.email=t@t -c user.name=t commit -qm fixture >/dev/null 2>&1

# The old name below is deliberate and must survive a future rename: this
# scrubber has to normalise both names for a comparison that spans one. A bulk
# substitution ate it once already, leaving the pattern matching one name twice.
#
# Timestamps, temporary paths and the tool's own name are the only things
# allowed to vary. Everything else, including counts, ordering, identities and
# wording, is the thing under comparison.
scrub() {
  sed -E \
    -e 's/[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9:+-]+/TIMESTAMP/g' \
    -e "s#$WORK#WORK#g" -e "s#$RULES#RULES#g" -e "s#$SESSION_DIR#SESSION#g" \
    -e 's#/var/folders/[^ "]*#TMP#g' -e 's#/private/tmp/[^ "]*#TMP#g' \
    -e 's#\.\./tmp\.[A-Za-z0-9]+#TMPREL#g' \
    -e 's/(codesweep|resweep)/TOOLNAME/g'
}

run() {
  local label="$1"; shift
  { "$TOOL" "$@" --root "$WORK" 2>&1; printf 'EXIT=%s\n' "$?"; } | scrub > "$WORK/out-$label.txt"
  if [ "$MODE" = record ]; then
    cp "$WORK/out-$label.txt" "$OUT/$label.txt"
    printf '  recorded %s\n' "$label"
  else
    if diff -q "$OUT/$label.txt" "$WORK/out-$label.txt" >/dev/null 2>&1; then
      printf '  ok   %s\n' "$label"
    else
      printf '  FAIL %s\n' "$label"
      diff "$OUT/$label.txt" "$WORK/out-$label.txt" | head -12
      FAILED=1
    fi
  fi
}

FAILED=0
run census-new census s --rule "$RULES/catch.yml" --scope src --question "swallowed?"
SITE="$("$TOOL" next s --root "$WORK" | python3 -c "import json,sys; print(json.load(sys.stdin)['sites'][0]['site_id'])")"
run next-batch next s --limit 2
run verdict-one verdict s --site "$SITE" --verdict violation --note "returns an empty array" --method "read the source"
run status-partial status s
run surfaces-empty surfaces s
run surfaces-propose surfaces s --propose
run recheck-none recheck s
run manifest manifest s
run list list
run census-rerun census s --rule "$RULES/catch.yml" --scope src
run bad-verdict verdict s --site "$SITE" --verdict pass --note ""
run bad-sweep status nosuchsweep
run bad-scope census other --rule "$RULES/catch.yml" --scope nowhere --question q

if [ "$MODE" = record ]; then
  printf '\nrecorded %s outputs into tests/reference\n' "$(ls "$OUT" | wc -l | tr -d ' ')"
else
  [ "$FAILED" -eq 0 ] && printf '\nevery command matches the reference\n' || printf '\nsome commands differ from the reference\n'
  exit "$FAILED"
fi
