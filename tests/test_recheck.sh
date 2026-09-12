#!/usr/bin/env bash
# Second looks: whether the judging was any good, as opposed to whether every
# site was looked at once.
#
# Every assertion compares against a number derived by reading the fixture.
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

export RESWEEP_SESSION_ID="recheck-$$-$(date +%s)"
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

# Weighted the way a real sweep is: mostly sites that turn out fine, a few that
# do not. An evenly split fixture would not reveal a sampler biased by verdict.
mkdir -p "$WORK/src"
cd "$WORK" && git init -q .
for i in 1 2 3 4 5 6 7 8; do
  printf 'export function f%s() { try { a(); } catch (e) { throw e; } }\n' "$i" > "src/f$i.ts"
done
# The two that genuinely swallow, and the one the second look will argue about.
printf 'export function g() { try { a(); } catch (e) { return []; } }\n' > src/g.ts
printf 'export function h() { try { a(); } catch (e) { console.warn(e); } }\n' > src/h.ts
git add -A >/dev/null 2>&1
git -c user.email=t@t -c user.name=t commit -qm fixture >/dev/null 2>&1

cs() { "$RESWEEP" "$@" --root "$WORK"; }
cs census s --rule "$RULES/catch.yml" --scope src --question "swallowed?" >/dev/null 2>&1

IDS="$(cs next s --limit 20 | python3 -c "import json,sys; [print(x['site_id']) for x in json.load(sys.stdin)['sites']]")"
COUNT="$(echo "$IDS" | wc -l | tr -d ' ')"
check "ten sites enumerated" "10" "$COUNT"

printf '\nnothing rechecked reads as nothing checked, not as a clean bill\n'
while read -r id; do cs verdict s --site "$id" --verdict pass --note "rethrows" --method "read the source" >/dev/null; done <<< "$IDS"
check "all judged" "0" "$(cs status s | jqp "d['coverage']['unjudged']")"
check "rechecked is zero" "0" "$(cs status s | jqp "d['coverage']['rechecked']")"
check "and it says so in words" "True" "$(cs status s | jqp "'recheck_note' in d")"
check "one method earns the caveat" "True" "$(cs status s | jqp "'method_note' in d")"

printf '\nthe second look shows nothing of the first judgement\n'
OFFER="$(cs recheck s --limit 3)"
check "three offered" "3" "$(echo "$OFFER" | jqp "len(d['sites'])")"
check "no verdict anywhere in the payload" "0" "$(echo "$OFFER" | grep -cE '"(pass|violation|na)"' || true)"
check "no note from the first judgement" "0" "$(echo "$OFFER" | grep -c 'rethrows' || true)"
check "no method from the first judgement" "0" "$(echo "$OFFER" | grep -c 'read the source' || true)"
check "the same sample comes back" "$(cs recheck s --limit 3 | jqp "[x['site_id'] for x in d['sites']]")" "$(cs recheck s --limit 3 | jqp "[x['site_id'] for x in d['sites']]")"

printf '\nagreement and disagreement are counted, and the first survives\n'
A="$(echo "$IDS" | head -1)"
B="$(echo "$IDS" | sed -n 2p)"
FIRST_A="$(cs status s >/dev/null; cs manifest s | grep "$A" || true)"
cs verdict s --second-opinion --site "$A" --verdict pass --note "same conclusion, different words" --method "ran the tests" >/dev/null
check "one checked, none disagreeing" "0" "$(cs status s | jqp "d['coverage']['disagreed']")"
check "a differing note is not a disagreement" "1" "$(cs status s | jqp "d['coverage']['rechecked']")"
cs verdict s --second-opinion --site "$B" --verdict violation --note "the caller cannot see it" --method "ran the tests" >/dev/null
check "two checked" "2" "$(cs status s | jqp "d['coverage']['rechecked']")"
check "one disagreement" "1" "$(cs status s | jqp "d['coverage']['disagreed']")"
check "the disagreeing site is named" "$B" "$(cs status s | jqp "d['coverage']['disagreeing_sites'][0]")"
check "the first judgement is untouched" "$FIRST_A" "$(cs manifest s | grep "$A" || true)"

printf '\ntwo methods clears the caveat, and a disagreement does not block the claim\n'
check "both methods listed" "2" "$(cs status s | jqp "len(d['coverage']['methods_used'])")"
check "no caveat now" "False" "$(cs status s | jqp "'method_note' in d")"
check "no recheck note now" "False" "$(cs status s | jqp "'recheck_note' in d")"
check "still complete despite the disagreement" "True" "$(cs status s | jqp "d['complete']")"

printf '\nthe refusals\n'
check "a second second opinion is refused" "1" "$(cs verdict s --second-opinion --site "$A" --verdict na --note x 2>&1 >/dev/null | grep -c 'already has a second opinion')"
UNJUDGED="$(echo "$IDS" | tail -1)"
check "an unknown site is refused" "1" "$(cs recheck s --site nosuchsite 2>&1 >/dev/null | grep -c 'not available to recheck')"
check "an empty note is refused" "1" "$(cs verdict s --second-opinion --site "$UNJUDGED" --verdict pass --note '' 2>&1 >/dev/null | grep -c 'not a judgement')"
# Refused by the argument parser before the command sees it, which lists the
# permitted set in its own words. Asserted as behaviour rather than wording, so
# the check does not break if either layer rephrases.
cs verdict s --second-opinion --site "$UNJUDGED" --verdict maybe --note x >/dev/null 2>&1
check "an invalid verdict is refused" "2" "$?"
check "and nothing was recorded for it" "2" "$(cs status s | jqp "d['coverage']['rechecked']")"

printf '\nmethods differing only in spacing and case count as one\n'
cs verdict s --site "$UNJUDGED" --verdict pass --note "n" --method "  Read The Source " >/dev/null
check "still two distinct methods" "2" "$(cs status s | jqp "len(d['coverage']['methods_used'])")"

printf '\nre-judging settles a dispute\n'
cs verdict s --site "$B" --verdict violation --note "the second look was right" --method "read the source again" >/dev/null
check "the dispute is cleared" "0" "$(cs status s | jqp "d['coverage']['disagreed']")"
check "and it is no longer counted as rechecked" "1" "$(cs status s | jqp "d['coverage']['rechecked']")"

printf '\na site whose code changed loses both records\n'
printf 'export function f1() { return 1; }\n' > "$WORK/src/f1.ts"
cs census s --rule "$RULES/catch.yml" --scope src >/dev/null 2>&1
check "the rechecked site is gone" "0" "$(cs status s | jqp "sum(1 for x in d['coverage']['disagreeing_sites'] if x=='$A')")"

printf '\nnothing is written into the repository being swept\n'
check "working tree unchanged apart from the edit" "1" "$(cd "$WORK" && git status --porcelain | wc -l | tr -d ' ')"

printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
