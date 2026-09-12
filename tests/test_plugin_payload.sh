#!/usr/bin/env bash
# Can a plugin carry an executable, and does the host select among builds?
#
# The port's distribution shape rests on the answers, so they are a check rather
# than a note in a document. If a future host stops preserving the executable
# bit, or grows a mechanism for choosing a build, this suite says so.
#
# It installs a plugin into the operator's own configuration and removes it
# again. Nothing else here touches anything outside a temporary directory.
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
PROBE_MARKET="resweep-payload-probe-market"
PROBE_PLUGIN="resweep-payload-probe"
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
  printf '  SKIP the host is not on PATH; the payload question needs it\n'
  printf '\n0 passed, 0 failed, 1 skipped\n'
  exit 0
fi

MARKET="$(mktemp -d)"
cleanup() {
  claude plugin uninstall "$PROBE_PLUGIN" >/dev/null 2>&1
  claude plugin marketplace remove "$PROBE_MARKET" >/dev/null 2>&1
  rm -rf "$MARKET"
}
trap cleanup EXIT

PLUGIN="$MARKET/plugins/$PROBE_PLUGIN"
mkdir -p "$PLUGIN/.claude-plugin" "$PLUGIN/bin" "$PLUGIN/skills/$PROBE_PLUGIN" "$MARKET/.claude-plugin"

# A script rather than a compiled binary. The questions are about the file mode
# and the invocation path, and neither depends on the payload being machine
# code; using a script keeps the suite free of a toolchain.
cat > "$PLUGIN/bin/payload" <<'EOF'
#!/bin/sh
echo "payload ran"
EOF
chmod +x "$PLUGIN/bin/payload"
SOURCE_SUM="$(shasum -a 256 "$PLUGIN/bin/payload" | cut -d' ' -f1)"

cat > "$PLUGIN/.claude-plugin/plugin.json" <<EOF
{
  "name": "$PROBE_PLUGIN",
  "version": "0.1.0",
  "description": "Measures whether a plugin can carry and run an executable payload.",
  "author": { "name": "resweep" },
  "license": "Apache-2.0"
}
EOF

cat > "$PLUGIN/skills/$PROBE_PLUGIN/SKILL.md" <<'EOF'
---
name: resweep-payload-probe
description: Runs the executable a plugin carries. Use only when checking plugin payload delivery.
---
Run `${CLAUDE_PLUGIN_ROOT}/bin/payload` and report what it prints.
EOF

cat > "$MARKET/.claude-plugin/marketplace.json" <<EOF
{
  "name": "$PROBE_MARKET",
  "owner": { "name": "resweep" },
  "plugins": [
    { "name": "$PROBE_PLUGIN", "source": "./plugins/$PROBE_PLUGIN",
      "description": "Measures whether a plugin can carry and run an executable payload." }
  ]
}
EOF

printf '\nan executable survives the trip through the host\n'
check "the manifest is valid" "1" \
  "$(claude plugin validate "$PLUGIN" >/dev/null 2>&1 && echo 1 || echo 0)"
claude plugin marketplace add "$MARKET" >/dev/null 2>&1
check "the marketplace is accepted" "1" \
  "$(claude plugin marketplace list 2>/dev/null | grep -c "$PROBE_MARKET" || true)"
claude plugin install "$PROBE_PLUGIN@$PROBE_MARKET" >/dev/null 2>&1
check "the plugin installs" "1" \
  "$(claude plugin list 2>/dev/null | grep -c "$PROBE_PLUGIN@$PROBE_MARKET" || true)"

INSTALLED="$(find "$HOME/.claude/plugins/cache/$PROBE_MARKET" -name payload -type f 2>/dev/null | head -1)"
check "the payload arrives" "1" "$([ -n "$INSTALLED" ] && echo 1 || echo 0)"
if [ -n "$INSTALLED" ]; then
  check "the executable bit survives" "1" "$([ -x "$INSTALLED" ] && echo 1 || echo 0)"
  check "the bytes are unchanged" "$SOURCE_SUM" "$(shasum -a 256 "$INSTALLED" | cut -d' ' -f1)"
  check "it runs from the plugin path" "payload ran" "$("$INSTALLED" 2>&1)"
  # The path an agent is given is the plugin root plus bin/<name>. Anything
  # deeper would mean the host rearranged the tree.
  check "it sits directly under the plugin root" "bin/payload" \
    "$(printf '%s' "$INSTALLED" | sed -E 's#.*/[0-9]+\.[0-9]+\.[0-9]+/##')"
fi

printf '\nthe host still offers no way to choose among builds\n'
# A manifest declaring a platform this machine is not. If the host selected on
# platform at all, this plugin would be refused or skipped. It is neither, which
# is what "no selection mechanism" means in practice rather than in the schema.
python3 - "$PLUGIN/.claude-plugin/plugin.json" <<'FOREIGN_PLATFORM'
import json, sys
d = json.load(open(sys.argv[1]))
d["os"] = ["plan9"]
d["cpu"] = ["vax"]
json.dump(d, open(sys.argv[1], "w"), indent=2)
FOREIGN_PLATFORM
check "a manifest naming another platform still validates" "1" \
  "$(claude plugin validate "$PLUGIN" >/dev/null 2>&1 && echo 1 || echo 0)"
claude plugin uninstall "$PROBE_PLUGIN" >/dev/null 2>&1
claude plugin marketplace update "$PROBE_MARKET" >/dev/null 2>&1
claude plugin install "$PROBE_PLUGIN@$PROBE_MARKET" >/dev/null 2>&1
check "and it installs anyway" "1" \
  "$(claude plugin list 2>/dev/null | grep -c "$PROBE_PLUGIN@$PROBE_MARKET" || true)"
FOREIGN="$(find "$HOME/.claude/plugins/cache/$PROBE_MARKET" -name payload -type f 2>/dev/null | head -1)"
check "and its payload runs on the wrong platform" "payload ran" \
  "$([ -n "$FOREIGN" ] && "$FOREIGN" 2>&1)"

printf '\n%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
