#!/usr/bin/env bash
# Surfaces: the declared list of parts of the system an audit intends to
# examine, and the completeness claim that is relative to it.
#
# Every assertion compares against a number or a name derived by reading the
# fixture, never against whatever the tool produced.
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

export RESWEEP_SESSION_ID="surfaces-$$-$(date +%s)"
SESSION_DIR="$(python3 -c 'import os,tempfile; print(os.path.join(tempfile.gettempdir(), "resweep", os.environ["RESWEEP_SESSION_ID"]))')"

WORK="$(mktemp -d)"
RULES="$(mktemp -d)"
cleanup() { rm -rf "$WORK" "$RULES" "$SESSION_DIR"; }
trap cleanup EXIT

# The rule lives outside the scope so the sweep can reach a genuinely complete
# state; a rule file inside the scope is itself source no rule reaches.
cat > "$RULES/catch.yml" <<'EOF'
id: ts-catch
language: typescript
rule:
  kind: catch_clause
EOF

# A realistic project shape: two packages with their own manifests, automated
# checks, hooks, documentation, deployment config, and a vendored directory
# that must never be proposed.
mkdir -p "$WORK/packages/api/src" "$WORK/packages/web" "$WORK/.github/workflows" \
         "$WORK/docs" "$WORK/.husky" "$WORK/node_modules/dep"
cd "$WORK" && git init -q .
printf 'node_modules/\n' > .gitignore
echo '{}' > packages/api/package.json
echo '{}' > packages/web/package.json
printf 'export function a() { try { b(); } catch (e) { return []; } }\n' > packages/api/src/index.ts
printf 'on: push\n' > .github/workflows/ci.yml
printf '#!/bin/sh\n' > .husky/pre-commit
printf '# docs\n' > docs/readme.md
printf 'name = "x"\n' > wrangler.toml
printf 'export function v() { try { c(); } catch (e) { throw e; } }\n' > node_modules/dep/v.ts
git add -A >/dev/null 2>&1
git -c user.email=t@t -c user.name=t commit -qm fixture >/dev/null 2>&1

cs() { "$RESWEEP" "$@" --root "$WORK"; }

printf '\nthe list starts empty, and an empty list is not a complete one\n'
cs census s --rule "$RULES/catch.yml" --scope packages/api/src --question "swallowed?" >/dev/null 2>&1
check "no list declared" "False" "$(cs surfaces s | jqp "d['declared_list']")"
check "and status says so" "True" "$(cs status s | jqp "'surfaces_note' in d")"

printf '\nparts are proposed from evidence, and ignored directories are not\n'
PROPOSED="$(cs surfaces s --propose)"
check "packages proposed separately" "2" "$(echo "$PROPOSED" | jqp "sum(1 for c in d['proposed'] if c['name'].startswith('package:'))")"
check "automated checks proposed" "True" "$(echo "$PROPOSED" | jqp "any(c['name']=='automated checks' for c in d['proposed'])")"
check "commit hooks proposed" "True" "$(echo "$PROPOSED" | jqp "any(c['name']=='commit hooks' for c in d['proposed'])")"
check "documentation proposed" "True" "$(echo "$PROPOSED" | jqp "any(c['name']=='documentation' for c in d['proposed'])")"
check "deployed configuration proposed" "True" "$(echo "$PROPOSED" | jqp "any(c['name']=='deployed configuration' for c in d['proposed'])")"
check "nothing from an ignored directory" "False" "$(echo "$PROPOSED" | jqp "any('node_modules' in str(c) for c in d['proposed'])")"
check "every candidate names its evidence" "True" "$(echo "$PROPOSED" | jqp "all(c['evidence'] for c in d['proposed'])")"
check "the proposal says it is incomplete" "True" "$(echo "$PROPOSED" | jqp "'note' in d")"

printf '\ndeclaring, refusing a duplicate, and refusing an unknown name\n'
cs surfaces s --add "worker source" --scope packages/api >/dev/null
cs surfaces s --add "deployed secrets" >/dev/null
check "two declared" "2" "$(cs surfaces s | jqp "d['surfaces_declared']")"
check "a part with no path is allowed" "None" "$(cs surfaces s | jqp "[x['scope'] for x in d['surfaces'] if x['name']=='deployed secrets'][0]")"
cs surfaces s --add "worker source" >/dev/null 2>&1
check "duplicate refused, still two" "2" "$(cs surfaces s | jqp "d['surfaces_declared']")"
cs surfaces s --examined "nope" >/dev/null 2>&1
check "unknown name refused" "1" "$(cs surfaces s --examined nope 2>&1 >/dev/null | grep -c 'no surface named')"

printf '\nan unexamined part blocks the claim, and every reason is given\n'
check "not complete" "False" "$(cs status s | jqp "d['complete']")"
check "the unexamined part is named" "True" "$(cs status s | jqp "any('worker source' in r for r in d['incomplete_because'])")"
check "the unjudged sites are also named" "True" "$(cs status s | jqp "any('no verdict' in r for r in d['incomplete_because'])")"
check "both reasons at once" "2" "$(cs status s | jqp "len(d['incomplete_because'])")"

printf '\njudging every site is not enough while a part is outstanding\n'
SITE="$(cs next s | jqp "d['sites'][0]['site_id']")"
cs verdict s --site "$SITE" --verdict violation --note "returns an empty array" >/dev/null
check "sites all judged" "0" "$(cs status s | jqp "d['coverage']['unjudged']")"
check "still not complete" "False" "$(cs status s | jqp "d['complete']")"
check "one reason left, the part" "1" "$(cs status s | jqp "len(d['incomplete_because'])")"

printf '\nexamining the parts completes it\n'
cs surfaces s --examined "worker source" >/dev/null
cs surfaces s --examined "deployed secrets" >/dev/null
check "complete" "True" "$(cs status s | jqp "d['complete']")"
check "no absent-list note now" "False" "$(cs status s | jqp "'surfaces_note' in d")"

printf '\na part added after a later pass is marked, and stays marked\n'
check "none late yet" "0" "$(cs status s | jqp "len(d['coverage']['surfaces_added_late'])")"
cs census s --rule "$RULES/catch.yml" --scope packages/api/src >/dev/null 2>&1
cs surfaces s --add "automated checks" >/dev/null
check "the late one is named" "automated checks" "$(cs status s | jqp "d['coverage']['surfaces_added_late'][0]")"
cs surfaces s --examined "automated checks" >/dev/null
check "still marked after being examined" "automated checks" "$(cs status s | jqp "d['coverage']['surfaces_added_late'][0]")"
check "the report says the list grew" "1" "$(cs report s 2>/dev/null | grep -c 'This list grew during the audit')"
check "the report lists every part" "3" "$(cs report s 2>/dev/null | sed -n '/^| Part |/,/^$/p' | grep -c '^| [a-z]')"

printf '\nremoving the last part reads as no list, not as a complete one\n'
for s in "worker source" "deployed secrets" "automated checks"; do cs surfaces s --remove "$s" >/dev/null; done
check "declared_list is false again" "False" "$(cs surfaces s | jqp "d['declared_list']")"
check "and the absent-list note returns" "True" "$(cs status s | jqp "'surfaces_note' in d")"

printf '\na census over a part does not mark it examined\n'
cs surfaces s --add "worker source" --scope packages/api >/dev/null
cs census s --rule "$RULES/catch.yml" --scope packages/api/src >/dev/null 2>&1
check "still unexamined after a census over it" "False" "$(cs surfaces s | jqp "d['surfaces'][0]['examined']")"

printf '\nnothing is written into the repository being swept\n'
check "working tree unchanged" "0" "$(cd "$WORK" && git status --porcelain | grep -vc '^$' || true)"

printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
