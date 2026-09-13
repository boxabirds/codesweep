#!/usr/bin/env bash
# The rename, checked against the tool as it was before the rename.
#
# A rename is the easiest change to get wrong quietly. Every assertion here
# compares the new name against the old artefact rather than against a value
# written down by hand, so a substitution that ate something real fails rather
# than being re-recorded as the new expectation.
#
# Not covered here, deliberately:
#   the five-suite run (TC-12/TC-30) belongs to run-all.sh, which runs this
#   file; calling it from inside would recurse.
#   the trigger probe round (TC-21 to TC-24) costs model time and is run by
#   hand.
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
NEW="$REPO/bin/resweep"
SHIM="$REPO/bin/codesweep"

# The last commit at which bin/codesweep was the tool itself rather than the
# shim. Named explicitly because the comparison is against one specific
# historical artefact, and "the parent of the rename" stops being true the
# moment anything is rebased.
PRE_RENAME_COMMIT="80187f6"

# A launcher that has grown a second job is no longer a launcher. It finds the
# build, says so clearly when there is not one for this machine, and execs.
LAUNCHER_MAX_LINES=55
LEDGER_SCHEMA_VERSION=4

PASS=0
FAIL=0
SKIP=0

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

export RESWEEP_SESSION_ID="rename-$$-$(date +%s)"
export CODESWEEP_SESSION_ID="$RESWEEP_SESSION_ID"
NEW_SESSION="$(python3 -c 'import os,tempfile; print(os.path.join(tempfile.gettempdir(), "resweep", os.environ["RESWEEP_SESSION_ID"]))')"
OLD_SESSION="$(python3 -c 'import os,tempfile; print(os.path.join(tempfile.gettempdir(), "codesweep", os.environ["CODESWEEP_SESSION_ID"]))')"
WORK="$(mktemp -d)"
RULES="$(mktemp -d)"
BARE="$(mktemp -d)"
cleanup() { rm -rf "$WORK" "$RULES" "$BARE" "$NEW_SESSION" "$OLD_SESSION"; }
trap cleanup EXIT

OLD="$WORK/old-tool"
# The pre-rename tool is Python and shells out to a separately installed
# matching engine. The port needs neither, so a machine without them can still
# run everything else here.
if git -C "$REPO" show "$PRE_RENAME_COMMIT:bin/codesweep" > "$OLD" 2>/dev/null &&
   command -v python3 >/dev/null 2>&1 &&
   command -v ast-grep >/dev/null 2>&1; then
  chmod +x "$OLD"
  HAVE_OLD=1
else
  HAVE_OLD=0
fi

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

# Both names collapse to one token, so a difference in wording survives the
# comparison and a difference in name does not. That is the whole point.
# Runs of spaces are squeezed as well. Argparse indents its continuation lines
# to the width of the program name, so the two names differ by two columns on
# every wrapped line. That is the name's length, not its content, and the thing
# under comparison is which words and flags exist.
norm() {
  sed -E -e 's/(codesweep|resweep)/TOOLNAME/g' -e "s#$WORK#WORK#g" -e "s#$RULES#RULES#g" \
    -e 's/  +/ /g'
}

# Help text additionally has its line breaks removed. Argparse wraps to the
# terminal width, so a shorter name lets a sentence fit where it used to wrap.
# Where the break falls is the name's length again; which words and flags are
# there is what matters.
norm_flow() { norm | tr '\n' ' ' | sed -E 's/  +/ /g'; }

# ---------------------------------------------------------------------------
printf '\nrefusals read the same and fail the same way\n'

# The full --help text is no longer compared here. It was argparse's rendering
# on both sides when this suite was written; the port renders its own, and the
# difference is the port's rather than the rename's. What the recorded
# reference pins, and what the guidance quotes, is the refusals, so those are
# what is compared.

if [ "$HAVE_OLD" -eq 0 ]; then
  printf '  SKIP no pre-rename tool at %s; the refusal comparison needs history\n' "$PRE_RENAME_COMMIT"
  SKIP=$((SKIP + 1))
else
  # Each tool keeps its ledger under its own name, so both need a census of
  # their own. Comparing a refusal against a tool that has no ledger compares
  # nothing.
  "$NEW" census s --rule "$RULES/catch.yml" --scope src --question "swallowed?" --root "$WORK" >/dev/null 2>&1
  CODESWEEP_SESSION_ID="$RESWEEP_SESSION_ID" python3 "$OLD" census s --rule "$RULES/catch.yml" --scope src --question "swallowed?" --root "$WORK" >/dev/null 2>&1
  SITE="$("$NEW" next s --root "$WORK" | jqp "d['sites'][0]['site_id']")"
  check "site identity does not depend on the tool's name" "$SITE" \
    "$(CODESWEEP_SESSION_ID="$RESWEEP_SESSION_ID" python3 "$OLD" next s --root "$WORK" | jqp "d['sites'][0]['site_id']")"

  refuse() {
    local at="$1"; shift
    local out rc
    if [ "$at" = "old" ]; then
      out="$(CODESWEEP_SESSION_ID="$RESWEEP_SESSION_ID" python3 "$OLD" "$@" --root "$WORK" 2>&1 | norm)"
    else
      out="$("$NEW" "$@" --root "$WORK" 2>&1 | norm)"
    fi
    rc="${PIPESTATUS[0]}"
    printf '%s\nEXIT=%s\n' "$out" "$rc"
  }
  for case_name in empty-note unknown-sweep bad-scope; do
    case "$case_name" in
      empty-note) set -- verdict s --site "$SITE" --verdict pass --note "" ;;
      unknown-sweep) set -- status nosuchsweep ;;
      bad-scope) set -- census other --rule "$RULES/catch.yml" --scope nowhere --question q ;;
    esac
    A="$(refuse old "$@")"
    B="$(refuse new "$@")"
    if [ "$A" = "$B" ]; then
      printf '  ok   %s refusal and exit code\n' "$case_name"
      PASS=$((PASS + 1))
    else
      printf '  FAIL %s refusal differs beyond the name\n' "$case_name"
      diff <(printf '%s\n' "$A") <(printf '%s\n' "$B") | head -8
      FAIL=$((FAIL + 1))
    fi
  done
fi

printf '\na ledger written before the rename reads the same after it\n'

if [ "$HAVE_OLD" -eq 0 ]; then
  printf '  SKIP no pre-rename tool; ledger carry-over needs one to write the ledger\n'
  SKIP=$((SKIP + 1))
else
  rm -rf "$NEW_SESSION" "$OLD_SESSION"
  CODESWEEP_SESSION_ID="$RESWEEP_SESSION_ID" python3 "$OLD" census carried --rule "$RULES/catch.yml" --scope src --question "swallowed?" --root "$WORK" >/dev/null 2>&1
  OLD_SITE="$(CODESWEEP_SESSION_ID="$RESWEEP_SESSION_ID" python3 "$OLD" next carried --root "$WORK" | jqp "d['sites'][0]['site_id']")"
  CODESWEEP_SESSION_ID="$RESWEEP_SESSION_ID" python3 "$OLD" verdict carried --site "$OLD_SITE" --verdict violation --note "returns an empty array" --method "read the source" --root "$WORK" >/dev/null 2>&1
  BEFORE="$(CODESWEEP_SESSION_ID="$RESWEEP_SESSION_ID" python3 "$OLD" status carried --root "$WORK" | norm)"

  DB="$(ls "$OLD_SESSION"/*.db 2>/dev/null | head -1)"
  check "the old tool wrote a ledger" "1" "$([ -f "$DB" ] && echo 1 || echo 0)"
  check "schema version is unchanged by the rename" "$LEDGER_SCHEMA_VERSION" \
    "$(python3 -c "import sqlite3,sys; print(sqlite3.connect(sys.argv[1]).execute(\"select value from meta where key='schema_version'\").fetchone()[0])" "$DB")"
  # If the name never enters the file, no ledger can be name-specific, whatever
  # else changes later.
  check "neither name appears anywhere in the ledger" "0" \
    "$(strings "$DB" 2>/dev/null | grep -cE 'codesweep|resweep' || true)"

  mkdir -p "$NEW_SESSION"
  cp "$DB" "$NEW_SESSION/$(basename "$DB")"
  AFTER="$("$NEW" status carried --root "$WORK" | norm)"
  if [ "$BEFORE" = "$AFTER" ]; then
    printf '  ok   every count survives the change of tool\n'
    PASS=$((PASS + 1))
  else
    printf '  FAIL the carried ledger reads differently\n'
    diff <(printf '%s\n' "$BEFORE") <(printf '%s\n' "$AFTER") | head -8
    FAIL=$((FAIL + 1))
  fi
  check "the sweep is still listed" "1" "$("$NEW" list --root "$WORK" | jqp "sum(1 for s in d if s['sweep']=='carried')")"
  check "the judgement carried too" "1" "$("$NEW" status carried --root "$WORK" | jqp "d['coverage']['judged']")"
fi

# ---------------------------------------------------------------------------
printf '\nthe retired name is gone, and so is the shim that carried it\n'

# The shim forwarded the old name to the new one so that someone mid-audit did
# not learn about a rename by having their tooling stop. It has now outlived
# the implementation it forwarded to, and removing it was the deliberate later
# decision this story left open.
check "the retired name is not in the repository" "0" \
  "$([ -e "$SHIM" ] && echo 1 || echo 0)"
check "and nothing installs it" "0" \
  "$(grep -c 'bin/codesweep' "$REPO/install.sh" || true)"

printf '\nthe name is gone from the live surfaces and kept in the records\n'

check "the command carries the new name" "1" "$([ -x "$NEW" ] && echo 1 || echo 0)"
check "the skill directory carries the new name" "1" \
  "$([ -f "$REPO/skills/resweep/SKILL.md" ] && echo 1 || echo 0)"
check "the old skill directory is gone" "0" \
  "$([ -d "$REPO/skills/codesweep" ] && echo 1 || echo 0)"
check "the entry point is a launcher, not an implementation" "1" \
  "$([ "$(wc -l < "$NEW")" -le "$LAUNCHER_MAX_LINES" ] && grep -q 'exec "$BUILT"' "$NEW" && echo 1 || echo 0)"
check "version control shows a move, not a delete and an add" "1" \
  "$([ "$(git -C "$REPO" log --format=%h --follow -- bin/resweep | wc -l | tr -d ' ')" -gt 1 ] && echo 1 || echo 0)"

for f in install.sh .claude-plugin/plugin.json README.md skills/resweep/SKILL.md \
         tests/test_resweep.sh tests/test_install.sh tests/test_surfaces.sh \
         tests/test_recheck.sh tests/test_against_ceetrix.sh tests/run-all.sh \
         evals/fixture/axios.sh; do
  check "no occurrence left in $f" "0" "$(grep -c 'codesweep' "$REPO/$f" || true)"
done

# These name it because they record what was true when written, and one of them
# is the document that argued for the rename.
for f in plans/v1.md plans/v2.md; do
  check "the record in $f is left alone" "kept" \
    "$([ "$(grep -c 'codesweep' "$REPO/$f" || true)" -gt 0 ] && echo kept || echo lost)"
done

# ---------------------------------------------------------------------------
printf '\nnothing but the name changed in what any command prints\n'
if "$HERE/capture-reference.sh" compare >/dev/null 2>&1; then
  printf '  ok   every captured command matches the pre-rename reference\n'
  PASS=$((PASS + 1))
else
  printf '  FAIL captured output differs from before the rename\n'
  "$HERE/capture-reference.sh" compare 2>&1 | grep -A6 FAIL | head -20
  FAIL=$((FAIL + 1))
fi

printf '\n%s passed, %s failed' "$PASS" "$FAIL"
[ "$SKIP" -gt 0 ] && printf ', %s skipped' "$SKIP"
printf '\n'
[ "$FAIL" -eq 0 ]
