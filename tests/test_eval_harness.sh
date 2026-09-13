#!/usr/bin/env bash
# Properties of the eval suite itself, checked without launching an agent.
#
# Everything here is about whether a number this suite produces can be trusted,
# rather than about what any number turned out to be. None of it costs model
# time, which is why it runs with the ordinary suites while the measurement runs
# by hand.
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
EVALS="$REPO/evals"
# The cases that must exist. Named, so deleting one is a failure rather than a
# smaller suite nobody notices.
EXPECTED_CASES="journey-axios-catch journey-private-defaults trigger-caller-finding trigger-rename trigger-scoped-audit trigger-single-literal"

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

printf '\nevery case is present and loads\n'
for name in $EXPECTED_CASES; do
  check "$name exists" "1" "$([ -d "$EVALS/$name" ] && echo 1 || echo 0)"
  HAS_PROMPT=0
  [ -f "$EVALS/$name/case.yaml" ] && HAS_PROMPT=1
  [ -f "$EVALS/$name/prompt.md" ] && HAS_PROMPT=1
  check "$name has a prompt" "1" "$HAS_PROMPT"
  # Graders live either as files under graders/ or inline in case.yaml. Both
  # are valid and the suite uses both, so a check that knew only one form would
  # be a rule about layout rather than about coverage.
  HAS_GRADER=0
  [ -n "$(ls "$EVALS/$name/graders/"*.md 2>/dev/null)" ] && HAS_GRADER=1
  grep -q '^graders:' "$EVALS/$name/case.yaml" 2>/dev/null && HAS_GRADER=1
  check "$name has at least one grader" "1" "$HAS_GRADER"
done

if command -v claude >/dev/null 2>&1; then
  # Loading happens before any agent starts, so a malformed case is caught here
  # for free. A filter matching nothing still loads every case.
  LOAD="$(cd "$EVALS/.." && claude plugin eval . --trust-plugin --case zzz-matches-nothing 2>&1)"
  check "every case parses" "0" "$(printf '%s' "$LOAD" | grep -ci 'invalid case\|failed to parse\|schema_version' || true)"
else
  printf '  SKIP the host is not on PATH, so cases cannot be loaded\n'
fi

printf '\nno prompt names the tool, or one arm is asked for something the other lacks\n'
for name in $EXPECTED_CASES; do
  BODY="$(cat "$EVALS/$name/case.yaml" "$EVALS/$name/prompt.md" 2>/dev/null | grep -iE '^\s*prompt:|^[A-Z]' || true)"
  check "$name asks a question, not an instruction to use a tool" "0" \
    "$(printf '%s' "$BODY" | grep -ci 'resweep\|codesweep\|use the skill\|run the command' || true)"
done

printf '\nthe should-not-fire cases state both bounds\n'
# A bare maximum of zero was once reported as expecting a range of one to zero.
for name in trigger-caller-finding trigger-rename trigger-single-literal; do
  G="$EVALS/$name/graders/trigger.md"
  check "$name sets a minimum" "1" "$(grep -c '^min:' "$G" || true)"
  check "$name sets a maximum" "1" "$(grep -c '^max:' "$G" || true)"
done

printf '\nthe fired indicator is an indicator and not part of the score\n'
check "the audit case marks it with-only" "1" \
  "$(grep -c 'arm: with-only' "$EVALS/journey-axios-catch/graders/tool-fired.md" || true)"
check "the should-not-fire cases do not, since both arms must be checked" "0" \
  "$(grep -c 'arm: with-only' "$EVALS/trigger-rename/graders/trigger.md" || true)"

printf '\nthe runner refuses a result that is not a comparison\n'
SINGLE="$(mktemp -d)/one-arm.json"
printf '{"cases":[{"name":"x","arms":{"with":[{"score":1}]}}]}' > "$SINGLE"
python3 - "$SINGLE" <<'ARMS'
import json, sys
run = json.load(open(sys.argv[1]))
text = json.dumps(run)
arms = {n for n in ("with", "without") if f'"{n}"' in text}
raise SystemExit(0 if arms != {"with", "without"} else 1)
ARMS
check "a single-arm result is detected" "0" "$?"
BOTH="$(mktemp -d)/two-arms.json"
printf '{"cases":[{"name":"x","arms":{"with":[{"score":1}],"without":[{"score":1}]}}]}' > "$BOTH"
python3 - "$BOTH" <<'ARMS'
import json, sys
run = json.load(open(sys.argv[1]))
text = json.dumps(run)
arms = {n for n in ("with", "without") if f'"{n}"' in text}
raise SystemExit(0 if arms != {"with", "without"} else 1)
ARMS
check "a two-arm result is accepted" "1" "$?"
rm -rf "$(dirname "$SINGLE")" "$(dirname "$BOTH")"

printf '\nthe runner can be asked to break the description on purpose\n'
check "a control mode exists" "1" \
  "$([ "$(grep -c -- '--control' "$HERE/run-evals.sh" || true)" -ge 1 ] && echo 1 || echo 0)"
check "it says a pass there is a failure" "1" \
  "$(grep -ci 'a pass here is a failure' "$HERE/run-evals.sh" || true)"

printf '\nthe fixture is pinned, and absence is told from drift\n'
check "the pin is named in one place" "1" "$(grep -c '^AXIOS_PIN=' "$HERE/run-evals.sh" || true)"
check "absence skips rather than fails" "1" \
  "$(grep -c 'This is a skip and not a failure' "$HERE/run-evals.sh" || true)"
check "drift is a hard error" "1" \
  "$(grep -c 'this is drift, not absence' "$HERE/run-evals.sh" || true)"

printf '\nscored runs are measurements, not source\n'
check "results are not committed" "1" "$(grep -c 'evals/results' "$REPO/.gitignore" || true)"

printf '\n%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
