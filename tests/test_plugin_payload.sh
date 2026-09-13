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
REPO="$(cd "$HERE/.." && pwd)"
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

printf '\nthe guidance reaches the tool where the plugin put it\n'
# The path the guidance issues, run as written with the plugin root pointing at
# this repository. A bare name only works if someone arranged for it, which is
# what the change to a plugin-relative path removed.
GUIDED="$(CLAUDE_PLUGIN_ROOT="$REPO" bash -c 'RESWEEP_SESSION_ID=guided-$$ "${CLAUDE_PLUGIN_ROOT}/bin/resweep" list --root .' 2>&1)"
check "the plugin-relative path runs" "1" \
  "$(printf '%s' "$GUIDED" | grep -cE 'no ledger|\[' || true)"
check "the guidance issues that path and not a bare name" "0" \
  "$(grep -cE '^resweep --root' "$REPO/skills/resweep/SKILL.md" || true)"
check "and it issues it more than once" "1" \
  "$([ "$(grep -c 'CLAUDE_PLUGIN_ROOT}/bin/resweep' "$REPO/skills/resweep/SKILL.md")" -ge 9 ] && echo 1 || echo 0)"

printf '\nthe guidance stops before the protocol when the tool is missing\n'
# The failure observed live: an agent found no command and improvised the whole
# protocol by hand, which produces exactly the hand-picked candidate set this
# tool exists to replace.
check "there is a preflight step before step 1" "1" \
  "$(grep -c '^### 0\.' "$REPO/skills/resweep/SKILL.md" || true)"
check "it tells the reader to stop" "1" \
  "$(grep -ci 'stop and say so' "$REPO/skills/resweep/SKILL.md" || true)"
check "and not to search the codebase instead" "1" \
  "$(tr '\n' ' ' < "$REPO/skills/resweep/SKILL.md" | grep -ci 'do not fall back to searching' || true)"

MISSING="$(mktemp -d)"
mkdir -p "$MISSING/bin"
cp "$REPO/bin/resweep" "$MISSING/bin/resweep"
ABSENT="$("$MISSING/bin/resweep" list --root . 2>&1)"
"$MISSING/bin/resweep" list --root . >/dev/null 2>&1; ABSENT_RC=$?
check "a plugin with no build refuses" "1" "$ABSENT_RC"
check "and says how to get one" "1" \
  "$(printf '%s' "$ABSENT" | grep -c 'install.sh' || true)"
check "and names the toolchain it needs" "1" \
  "$(printf '%s' "$ABSENT" | grep -c 'rustup.rs' || true)"

printf '\na build for another machine says so, rather than failing as a format error\n'
WRONG="$(mktemp -d)"
mkdir -p "$WRONG/bin" "$WRONG/target/release"
cp "$REPO/bin/resweep" "$WRONG/bin/resweep"
# A real executable for a machine this is not. Written as an ELF header on
# macOS or a Mach-O header on Linux, so `file` reports a genuine foreign
# binary rather than an unidentifiable blob.
case "$(uname -s)" in
  Darwin) printf '\177ELF\002\001\001\000\000\000\000\000\000\000\000\000\002\000\076\000' > "$WRONG/target/release/resweep" ;;
  *)      printf '\317\372\355\376\014\000\000\001\000\000\000\000\002\000\000\000' > "$WRONG/target/release/resweep" ;;
esac
chmod +x "$WRONG/target/release/resweep"
FOREIGN_OUT="$("$WRONG/bin/resweep" list --root . 2>&1)"
"$WRONG/bin/resweep" list --root . >/dev/null 2>&1; FOREIGN_RC=$?
check "it refuses" "1" "$FOREIGN_RC"
check "it names this machine" "1" \
  "$(printf '%s' "$FOREIGN_OUT" | grep -c "$(uname -m)" || true)"
check "it describes the build it found" "1" \
  "$(printf '%s' "$FOREIGN_OUT" | grep -c 'the build is' || true)"
check "it says how to get the right one" "1" \
  "$(printf '%s' "$FOREIGN_OUT" | grep -c 'cargo build' || true)"
check "and never surfaces a bare format error" "0" \
  "$(printf '%s' "$FOREIGN_OUT" | grep -ci 'format error\|cannot execute\|permission denied' || true)"
rm -rf "$MISSING" "$WRONG"

printf '\nwith nothing else installed, a census still answers\n'
# The claim the whole port exists to make. PATH is cut to the shell and the
# core utilities, so neither a separately installed matching engine nor an
# interpreter is reachable.
BARE_PATH="/bin:/usr/bin"
check "no matching engine on the stripped path" "0" \
  "$(PATH="$BARE_PATH" command -v ast-grep >/dev/null 2>&1 && echo 1 || echo 0)"
CLEAN="$(mktemp -d)"
mkdir -p "$CLEAN/src"
printf 'export function a() { try { x(); } catch (e) { return []; } }\n' > "$CLEAN/src/a.ts"
printf 'export function b() { try { x(); } catch (e) { throw e; } }\n' > "$CLEAN/src/b.ts"
cat > "$CLEAN/catch.yml" <<'EOF'
id: ts-catch
language: typescript
rule:
  kind: catch_clause
EOF
CENSUS="$(PATH="$BARE_PATH" RESWEEP_SESSION_ID="clean-$$" "$REPO/target/release/resweep"   census s --rule "$CLEAN/catch.yml" --scope src --question "swallowed?" --root "$CLEAN" 2>&1)"
check "the census runs with nothing else installed" "2" \
  "$(printf '%s' "$CENSUS" | grep -oE '"sites_found": [0-9]+' | grep -oE '[0-9]+' || echo missing)"
rm -rf "$CLEAN" "${TMPDIR:-/tmp}/resweep/clean-$$"

printf '\n%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
