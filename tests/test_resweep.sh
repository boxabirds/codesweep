#!/usr/bin/env bash
# End-to-end tests for resweep against a fixture repository with a known,
# hand-counted number of candidate sites. Every assertion compares against a
# number derived by reading the fixture, not against whatever the tool produced.
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
RESWEEP="$HERE/../bin/resweep"
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

# Every run gets its own session id so the index lands in a directory no other
# run touches, and so a failed run leaves nothing that a later run inherits.
export RESWEEP_SESSION_ID="test-$$-$(date +%s)"
SESSION_DIR="$(python3 -c 'import os,tempfile; print(os.path.join(tempfile.gettempdir(), "resweep", os.environ["RESWEEP_SESSION_ID"]))')"

WORK="$(mktemp -d)"
cleanup() {
  rm -rf "$WORK"
  rm -rf "$SESSION_DIR"
}
trap cleanup EXIT
mkdir -p "$WORK/src"

# Fixture: 3 catch clauses. Two swallow, one rethrows.
cat > "$WORK/src/a.ts" <<'EOF'
export async function load(db: D1Database) {
  try {
    return await db.prepare('select 1').all();
  } catch (e) {
    return [];
  }
}

export async function save(db: D1Database) {
  try {
    return await db.prepare('insert').run();
  } catch (e) {
    throw e;
  }
}
EOF

cat > "$WORK/src/b.ts" <<'EOF'
export function parse(raw: string) {
  try {
    return JSON.parse(raw);
  } catch (e) {
    return null;
  }
}
EOF

cat > "$WORK/catch.yml" <<'EOF'
id: ts-catch
language: typescript
rule:
  kind: catch_clause
EOF

cat > "$WORK/swallow.yml" <<'EOF'
id: ts-swallow
language: typescript
rule:
  kind: catch_clause
  not:
    has:
      stopBy: end
      pattern: throw $E
EOF

echo "census enumerates every catch clause"
OUT=$("$RESWEEP" --root "$WORK" census all --rule "$WORK/catch.yml" --scope "$WORK/src" \
  --question "Does this catch clause hide a failure from the caller?")
check "sites found" 3 "$(echo "$OUT" | jqp 'd["sites_found"]')"
check "all new"     3 "$(echo "$OUT" | jqp 'd["sites_new"]')"
check "all unjudged" 3 "$(echo "$OUT" | jqp 'd["unjudged"]')"

echo "a narrowed rule excludes the rethrow"
OUT=$("$RESWEEP" --root "$WORK" census narrow --rule "$WORK/swallow.yml" --scope "$WORK/src" \
  --question "Does this catch clause hide a failure from the caller?")
check "narrowed sites" 2 "$(echo "$OUT" | jqp 'd["sites_found"]')"

echo "next hands out unjudged sites and nothing else"
OUT=$("$RESWEEP" --root "$WORK" next all --limit 2)
check "batch size" 2 "$(echo "$OUT" | jqp 'len(d["sites"])')"
check "remaining after batch" 1 "$(echo "$OUT" | jqp 'd["remaining_after_this_batch"]')"
check "context has a marker" "True" "$(echo "$OUT" | jqp '">" in d["sites"][0]["context"]')"

echo "an empty note is refused"
FIRST=$("$RESWEEP" --root "$WORK" next all --limit 1 | jqp 'd["sites"][0]["site_id"]')
"$RESWEEP" --root "$WORK" verdict all --site "$FIRST" --verdict pass --note "" >/dev/null 2>&1
check "exit code for empty note" 1 "$?"

echo "an unknown site id is refused"
"$RESWEEP" --root "$WORK" verdict all --site deadbeefdeadbeef --verdict pass --note "x" >/dev/null 2>&1
check "exit code for unknown site" 1 "$?"

echo "status reports incomplete until the queue is drained"
check "not complete yet" "False" "$("$RESWEEP" --root "$WORK" status all | jqp 'd["complete"]')"

echo "draining the queue makes coverage arithmetic close"
while true; do
  BATCH=$("$RESWEEP" --root "$WORK" next all --limit 2)
  COUNT=$(echo "$BATCH" | jqp 'len(d["sites"])')
  [ "$COUNT" = "0" ] && break
  # Judge only the matched lines, which `next` marks with a leading '>'. Reading
  # the whole context block would let a neighbouring site's `throw` bleed in,
  # which is the excerpt-bleed mistake the skill warns a judging agent about.
  echo "$BATCH" | python3 -c "
import json,sys
d=json.load(sys.stdin)
def matched(ctx):
    return '\n'.join(l for l in ctx.splitlines() if l.startswith('>'))
print(json.dumps([
  {'site_id': s['site_id'],
   'verdict': 'pass' if 'throw' in matched(s['context']) else 'violation',
   'note': 'judged in test from the matched lines only'}
  for s in d['sites']]))" | "$RESWEEP" --root "$WORK" verdict all --from-json - >/dev/null
done
ST=$("$RESWEEP" --root "$WORK" status all)
check "unjudged"   0 "$(echo "$ST" | jqp 'd["coverage"]["unjudged"]')"
check "judged"     3 "$(echo "$ST" | jqp 'd["coverage"]["judged"]')"
check "complete"   "True" "$(echo "$ST" | jqp 'd["complete"]')"
check "violations" 2 "$(echo "$ST" | jqp 'd["coverage"]["by_verdict"]["violation"]')"

echo "reformatting alone does not invalidate a verdict"
python3 - "$WORK/src/b.ts" <<'EOF'
import sys
p = sys.argv[1]
src = open(p).read().replace("  } catch (e) {", "  }\n  catch (e) {")
open(p, "w").write(src)
EOF
OUT=$("$RESWEEP" --root "$WORK" census all --rule "$WORK/catch.yml" --scope "$WORK/src")
check "reformat adds no sites" 0 "$(echo "$OUT" | jqp 'd["sites_new"]')"
check "reformat still complete" 0 "$(echo "$OUT" | jqp 'd["unjudged"]')"

echo "changing a site's code returns it to the queue"
python3 - "$WORK/src/b.ts" <<'EOF'
import sys
p = sys.argv[1]
src = open(p).read().replace("return null;", "return undefined;")
open(p, "w").write(src)
EOF
OUT=$("$RESWEEP" --root "$WORK" census all --rule "$WORK/catch.yml" --scope "$WORK/src")
check "changed site is new"      1 "$(echo "$OUT" | jqp 'd["sites_new"]')"
check "old site departed"        1 "$(echo "$OUT" | jqp 'd["sites_departed"]')"
check "one site needs rejudging" 1 "$(echo "$OUT" | jqp 'd["unjudged"]')"
check "untouched verdicts kept"  2 "$("$RESWEEP" --root "$WORK" status all | jqp 'd["coverage"]["judged"]')"

echo "deleting a file removes its sites from the arithmetic"
rm "$WORK/src/b.ts"
OUT=$("$RESWEEP" --root "$WORK" census all --rule "$WORK/catch.yml" --scope "$WORK/src")
check "sites after deletion" 2 "$(echo "$OUT" | jqp 'd["sites_found"]')"
check "complete again"       0 "$(echo "$OUT" | jqp 'd["unjudged"]')"

echo "changing the question on an existing sweep is refused"
"$RESWEEP" --root "$WORK" census all --rule "$WORK/catch.yml" --scope "$WORK/src" \
  --question "a completely different question" >/dev/null 2>&1
check "exit code for changed question" 1 "$?"

echo "a scope containing .tsx warns when only a typescript rule is given"
# This is the defect that shipped: language: typescript silently skips .tsx, so
# the sweep completes with a confident, clean, wrong report.
mkdir -p "$WORK/mixed"
cat > "$WORK/mixed/plain.ts" <<'EOF'
export function a() { try { f(); } catch (e) { return null; } }
EOF
cat > "$WORK/mixed/View.tsx" <<'EOF'
export function View() {
  try { return <div />; } catch (e) { return null; }
}
EOF
OUT=$("$RESWEEP" --root "$WORK" census tsxgap --rule "$WORK/catch.yml" --scope "$WORK/mixed" \
  --question "Does this catch clause hide a failure from the caller?")
check "ts-only rule finds one site"  1 "$(echo "$OUT" | jqp 'd["sites_found"]')"
check "warns about .tsx"       "True" "$(echo "$OUT" | jqp "'.tsx' in d.get('WARNING_uncovered_extensions', {})")"
check "counts the tsx files"         1 "$(echo "$OUT" | jqp 'd["WARNING_uncovered_extensions"][".tsx"]')"

cat > "$WORK/catch-tsx.yml" <<'EOF'
id: tsx-catch
language: tsx
rule:
  kind: catch_clause
EOF
OUT=$("$RESWEEP" --root "$WORK" census tsxgap --rule "$WORK/catch.yml" --rule "$WORK/catch-tsx.yml" \
  --scope "$WORK/mixed")
check "both rules find both sites" 2 "$(echo "$OUT" | jqp 'd["sites_found"]')"
check "warning cleared"       "False" "$(echo "$OUT" | jqp "'WARNING_uncovered_extensions' in d")"

echo "a relative scope resolves against --root, not the working directory"
OUT=$(cd / && "$RESWEEP" --root "$WORK" census relscope --rule "$WORK/catch.yml" --scope mixed \
  --question "Does this catch clause hide a failure from the caller?")
check "relative scope found the repo tree" 1 "$(echo "$OUT" | jqp 'd["sites_found"]')"

echo "--root works after the subcommand as well as before"
check "before subcommand" "True" "$("$RESWEEP" --root "$WORK" list | jqp "len(d) > 0")"
check "after subcommand"  "True" "$("$RESWEEP" list --root "$WORK" | jqp "len(d) > 0")"

echo "show re-reads a site after it has left the queue"
SITE=$("$RESWEEP" --root "$WORK" show all --site "$FIRST" | jqp 'd["site_id"]')
check "show returns the site"     "$FIRST" "$SITE"
check "show carries the verdict"  "True" "$("$RESWEEP" --root "$WORK" show all --site "$FIRST" | jqp 'd["verdict"] is not None')"

echo "the report states its own coverage and names its rules"
REPORT=$("$RESWEEP" --root "$WORK" report all)
check "report names the rule"    "True" "$(printf '%s' "$REPORT" | grep -q 'ts-catch' && echo True || echo False)"
check "report claims complete"   "True" "$(printf '%s' "$REPORT" | grep -q 'complete with respect to' && echo True || echo False)"
check "report inlines rule source" "True" "$(printf '%s' "$REPORT" | grep -q 'kind: catch_clause' && echo True || echo False)"
check "report states the syntactic limit" "True" "$(printf '%s' "$REPORT" | grep -q 'never with respect to a symbol' && echo True || echo False)"

echo "an incomplete sweep says so in its report"
INCOMPLETE=$("$RESWEEP" --root "$WORK" report narrow)
check "incomplete report warns" "True" "$(printf '%s' "$INCOMPLETE" | grep -q 'INCOMPLETE' && echo True || echo False)"

echo "the index lives in a session temp directory, never in the repository"
check "no .resweep in the swept repo" "False" "$([ -e "$WORK/.resweep" ] && echo True || echo False)"
check "index is under the session dir"  "True"  "$(ls "$SESSION_DIR"/*.db >/dev/null 2>&1 && echo True || echo False)"
check "index is named for the repo"     "True"  "$(ls "$SESSION_DIR" | grep -qE '^[A-Za-z0-9_-]+-[0-9a-f]{8}\.db$' && echo True || echo False)"

echo "two roots in one session get separate indexes"
WORK2="$(mktemp -d)"; mkdir -p "$WORK2/src"; cp "$WORK/src/a.ts" "$WORK2/src/a.ts"
"$RESWEEP" --root "$WORK2" census other --rule "$WORK/catch.yml" --scope "$WORK2/src" \
  --question "Does this catch clause hide a failure from the caller?" >/dev/null
check "two databases in one session" 2 "$(ls "$SESSION_DIR"/*.db | wc -l | tr -d ' ')"
check "the other root sees only its own sweeps" "other" "$("$RESWEEP" --root "$WORK2" list | jqp 'd[0]["sweep"]')"
rm -rf "$WORK2"

echo "a run with no session at all is refused, not silently shared"
env -u RESWEEP_SESSION_ID -u CLAUDE_CODE_SESSION_ID "$RESWEEP" --root "$WORK" list >/dev/null 2>&1
check "exit code with no session" 1 "$?"
NOSESS=$(env -u RESWEEP_SESSION_ID -u CLAUDE_CODE_SESSION_ID "$RESWEEP" --root "$WORK" list 2>&1)
check "the refusal names the override" "True" "$(printf '%s' "$NOSESS" | grep -q 'RESWEEP_SESSION_ID' && echo True || echo False)"

echo "a session id that could climb out of the temp root is refused"
RESWEEP_SESSION_ID="../../etc" "$RESWEEP" --root "$WORK" list >/dev/null 2>&1
check "exit code for traversal attempt" 1 "$?"

echo "manifest lists every live site, and its count matches the census"
# A second file with a different number of catch clauses, so that filtering the
# manifest to one file is a real test rather than a restatement of the total.
cat > "$WORK/src/c.ts" <<'EOF'
export function one(x: string) {
  try { return JSON.parse(x); } catch (e) { return null; }
}
EOF
SITES=$("$RESWEEP" --root "$WORK" census all --rule "$WORK/catch.yml" --scope "$WORK/src" | jqp 'd["sites_found"]')
MAN=$("$RESWEEP" --root "$WORK" manifest all)
check "manifest rows equal census count" "$SITES" "$(printf '%s\n' "$MAN" | grep -cE '^  [0-9a-f]{16}  ')"
check "manifest states the total"  "True" "$(printf '%s' "$MAN" | grep -q "of $SITES live sites" && echo True || echo False)"
check "manifest names its rules"   "True" "$(printf '%s' "$MAN" | grep -q 'rules: ts-catch' && echo True || echo False)"
check "manifest states the syntactic limit" "True" "$(printf '%s' "$MAN" | grep -q 'never a candidate' && echo True || echo False)"
check "manifest groups under files" "True" "$(printf '%s' "$MAN" | grep -q '^src/a.ts$' && echo True || echo False)"

echo "manifest can be scoped to one file"
# a.ts holds two catch clauses, c.ts holds one, so the two filters must differ
# from each other and from the total of three.
check "a.ts has two"        2 "$("$RESWEEP" --root "$WORK" manifest all --file src/a.ts | grep -cE '^  [0-9a-f]{16}  ')"
check "c.ts has one"        1 "$("$RESWEEP" --root "$WORK" manifest all --file src/c.ts | grep -cE '^  [0-9a-f]{16}  ')"
check "the two sum to all"  "$SITES" 3
"$RESWEEP" --root "$WORK" manifest all --file src/nope.ts >/dev/null 2>&1
check "unknown file is refused" 1 "$?"

echo "manifest can list only what is still unjudged"
check "unjudged count" "$("$RESWEEP" --root "$WORK" status all | jqp 'd["coverage"]["unjudged"]')" \
  "$("$RESWEEP" --root "$WORK" manifest all --unjudged | grep -cE '^  [0-9a-f]{16}  ')"

echo "a stale session directory is removed by a later census"
STALE="$(dirname "$SESSION_DIR")/test-stale-$$"
mkdir -p "$STALE" && touch -t 202001010000 "$STALE"
"$RESWEEP" --root "$WORK" census all --rule "$WORK/catch.yml" --scope "$WORK/src" >/dev/null
check "stale session removed" "False" "$([ -d "$STALE" ] && echo True || echo False)"
check "this session survived"  "True" "$([ -d "$SESSION_DIR" ] && echo True || echo False)"

printf '\n%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
