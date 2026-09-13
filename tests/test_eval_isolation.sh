#!/usr/bin/env bash
# Whether an eval run can see the operator's own installed plugins.
#
# Every other number this suite can produce depends on this. If the operator's
# environment leaks into the baseline arm, the arm without the plugin under test
# is not without a plugin, and the delta measures nothing.
#
# Isolation can only be proven against the thing it is isolating from, so this
# installs a real decoy into the operator's real configuration and then looks in
# the run's own trace to see what was offered. Nothing is mocked, and the decoy
# is removed afterwards whether the checks pass or not.
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
DECOY_MARKET="resweep-decoy-market"
DECOY_PLUGIN="resweep-decoy"
# Distinctive enough that finding it anywhere is unambiguous.
DECOY_SKILL="resweep-decoy-should-never-be-offered"
# One case, one run per arm, chosen because it is the cheapest that still starts
# two real agents.
PROBE_CASE="trigger-single-literal"

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

if ! command -v claude >/dev/null 2>&1; then
  printf '  SKIP the host is not on PATH\n\n0 passed, 0 failed, 1 skipped\n'
  exit 0
fi
if [ ! -x "$REPO/target/release/resweep" ] && ! command -v cargo >/dev/null 2>&1; then
  printf '  SKIP no build and no cargo to make one\n\n0 passed, 0 failed, 1 skipped\n'
  exit 0
fi

MARKET="$(mktemp -d)"
cleanup() {
  claude plugin uninstall "$DECOY_PLUGIN" >/dev/null 2>&1
  claude plugin marketplace remove "$DECOY_MARKET" >/dev/null 2>&1
  rm -rf "$MARKET"
}
trap cleanup EXIT

PLUGIN="$MARKET/plugins/$DECOY_PLUGIN"
mkdir -p "$PLUGIN/.claude-plugin" "$PLUGIN/skills/$DECOY_SKILL" "$MARKET/.claude-plugin"
cat > "$PLUGIN/.claude-plugin/plugin.json" <<EOF
{
  "name": "$DECOY_PLUGIN",
  "version": "0.1.0",
  "description": "A decoy installed to prove an eval run cannot see the operator's own plugins.",
  "author": { "name": "resweep" },
  "license": "Apache-2.0"
}
EOF
cat > "$PLUGIN/skills/$DECOY_SKILL/SKILL.md" <<EOF
---
name: $DECOY_SKILL
description: Never use this. It exists so a test can look for it and fail if it is ever offered to an eval run.
---
If you are reading this inside an eval run, the run is not isolated.
EOF
cat > "$MARKET/.claude-plugin/marketplace.json" <<EOF
{
  "name": "$DECOY_MARKET",
  "owner": { "name": "resweep" },
  "plugins": [
    { "name": "$DECOY_PLUGIN", "source": "./plugins/$DECOY_PLUGIN",
      "description": "A decoy installed to prove an eval run cannot see the operator's own plugins." }
  ]
}
EOF

printf '\nthe decoy is really installed, so the test is against something\n'
claude plugin marketplace add "$MARKET" >/dev/null 2>&1
claude plugin install "$DECOY_PLUGIN@$DECOY_MARKET" >/dev/null 2>&1
check "the decoy plugin is installed" "1" \
  "$(claude plugin list 2>/dev/null | grep -c "$DECOY_PLUGIN@$DECOY_MARKET" || true)"

printf '\nrunning one case, both arms, with the traces kept\n'
RESULT="$REPO/evals/results/isolation-run.json"
rm -f "$RESULT"
if ! EVAL_JSON="$RESULT" "$HERE/run-evals.sh" --case "$PROBE_CASE" --runs 1 --keep-temp >/dev/null 2>&1; then
  printf '  note: the case did not score, which does not matter here\n'
fi
[ -f "$RESULT" ] || RESULT="$REPO/evals/results/last-run.json"
check "a result was produced" "1" "$([ -f "$RESULT" ] && echo 1 || echo 0)"

printf '\nwhat each arm was actually offered\n'
python3 - "$RESULT" "$DECOY_SKILL" <<'INSPECT'
import json, sys

result_path, decoy = sys.argv[1], sys.argv[2]
run = json.load(open(result_path))
findings = []
for case in run.get("cases", []):
    for arm, runs in case.get("arms", {}).items():
        for r in runs:
            trace = r.get("tracePath")
            offered, plugins = None, None
            try:
                with open(trace) as f:
                    for line in f:
                        try:
                            event = json.loads(line)
                        except Exception:
                            continue
                        if event.get("type") == "system" and event.get("subtype") == "init":
                            offered = event.get("skills") or []
                            plugins = [p.get("name") for p in event.get("plugins") or []]
                            break
            except Exception as e:
                findings.append((arm, None, None, str(e)))
                continue
            findings.append((arm, offered, plugins, None))

if not findings:
    print("  FAIL no arm recorded what it was offered")
    raise SystemExit(1)

status = 0
for arm, offered, plugins, err in findings:
    if err is not None:
        print(f"  FAIL {arm}: its trace could not be read ({err})")
        status = 1
        continue
    if decoy in offered or any(decoy in p for p in plugins):
        print(f"  FAIL {arm} was offered the decoy: {offered}")
        status = 1
    else:
        print(f"  ok   {arm} was not offered the decoy ({len(offered)} skills)")
    # The plugin under test belongs in exactly one arm. If it is in both, the
    # baseline is not a baseline; if in neither, nothing was measured.
    has_it = any(p == "resweep" for p in plugins)
    if arm == "with" and not has_it:
        print(f"  FAIL the with arm did not carry the plugin under test: {plugins}")
        status = 1
    elif arm == "with":
        print("  ok   the with arm carried the plugin under test")
    if arm == "without" and has_it:
        print(f"  FAIL the without arm carried the plugin under test: {plugins}")
        status = 1
    elif arm == "without":
        print("  ok   the without arm carried no plugin")
raise SystemExit(status)
INSPECT
INSPECTED=$?
if [ "$INSPECTED" -eq 0 ]; then
  PASS=$((PASS + 4))
else
  FAIL=$((FAIL + 1))
fi

printf '\n%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
