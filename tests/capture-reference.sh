#!/usr/bin/env bash
# What every command prints, recorded before the port and compared after it.
#
# A port's definition of correct is sameness, and sameness can only be judged
# against something captured beforehand. A comparison written afterwards, from
# memory of what the tool used to do, quietly redefines every regression as an
# improvement.
#
# The recorded set was taken from the tool as it stood at 80187f6, the last
# commit before the rename. That makes one set serve two jobs: it is the
# baseline the rename had to preserve, and it is the baseline the Rust port has
# to reproduce. Re-record it only with RESWEEP_BIN pointing at that same
# artefact, or the baseline stops being a baseline.
#
#   git show 80187f6:bin/codesweep > /tmp/old && chmod +x /tmp/old
#   RESWEEP_BIN=/tmp/old tests/capture-reference.sh record
#   tests/capture-reference.sh compare
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
MODE="${1:-compare}"
OUT="$HERE/reference"
# The Rust binary by default, because that is now the tool the reference
# describes. The recording was taken from the Python tool at 80187f6 and the
# port reproduces all thirty-three, with one deliberate exception recorded in
# its own commit: file_discovery reports which ignore rules applied rather than
# which program was asked. The Python tool therefore differs from the reference
# in that one field, and will until it is removed.
TOOL="${RESWEEP_BIN:-$HERE/../target/release/resweep}"
RULES_DIR="$HERE/../rules"

# The second fixture is a real third-party repository at a fixed commit. A
# constructed fixture proves the mechanism and cannot prove the tool survives
# generated files, vendored bundles and a .gitignore that matters. The pin is
# what makes a byte-for-byte capture possible at all: the private monorepo the
# other suite uses changes daily, so frozen bytes over it would fail for the
# wrong reason within a day.
AXIOS_PIN=8092aee7240220aa7913d163187b7fba8d9e5a51
AXIOS_CACHE="${RESWEEP_FIXTURE_DIR:-$HOME/.cache/github/axios/axios}"

# Both names are set: the tool being run may be either side of the rename, and
# each reads its own.
export RESWEEP_SESSION_ID="reference-$$-$(date +%s)"
export CODESWEEP_SESSION_ID="$RESWEEP_SESSION_ID"
NEW_SESSION="$(python3 -c 'import os,tempfile; print(os.path.join(tempfile.gettempdir(), "resweep", os.environ["RESWEEP_SESSION_ID"]))')"
OLD_SESSION="$(python3 -c 'import os,tempfile; print(os.path.join(tempfile.gettempdir(), "codesweep", os.environ["CODESWEEP_SESSION_ID"]))')"
# The fixture directories are named rather than left as mktemp's random
# basename. The ledger's filename is built from the repository's directory
# name, so a random one would differ on every run and force the whole filename
# to be normalised away, hiding the naming scheme the port has to reproduce.
TMPROOT="$(mktemp -d)"
WORK="$TMPROOT/fixture"
RULES="$TMPROOT/rules"
BIG="$TMPROOT/big"
mkdir -p "$WORK" "$RULES"
cleanup() { rm -rf "$TMPROOT" "$NEW_SESSION" "$OLD_SESSION"; }
trap cleanup EXIT

if [ ! -x "$TOOL" ]; then
  if command -v cargo >/dev/null 2>&1; then
    printf 'building the port first\n'
    (cd "$HERE/.." && cargo build -p resweep --release --quiet) || {
      printf 'the port will not build, so there is nothing to compare\n'; exit 1; }
  else
    printf 'SKIP no binary at %s and no cargo to build one\n' "$TOOL"
    exit 0
  fi
fi

cat > "$RULES/catch.yml" <<'EOF'
id: ts-catch
language: typescript
rule:
  kind: catch_clause
EOF

# The pinned repository is entirely JavaScript, so the shipped TypeScript rules
# match nothing in it. Both cases are worth recording: the rule that matches
# nothing, because the warning it raises about uncovered extensions is the
# tool's most important output, and a rule that matches plenty, because a
# baseline of zero would be satisfied by a completely broken matcher.
cat > "$RULES/js-catch.yml" <<'EOF'
id: js-catch
language: javascript
rule:
  kind: catch_clause
EOF
cat > "$RULES/js-default.yml" <<'EOF'
id: js-default
language: javascript
rule:
  any:
    - pattern: $A || $B
    - pattern: $A ?? $B
EOF

mkdir -p "$WORK/src"
cd "$WORK" && git init -q .
printf 'export function a() { try { x(); } catch (e) { return []; } }\n' > src/a.ts
printf 'export function b() { try { x(); } catch (e) { throw e; } }\n' > src/b.ts
git add -A >/dev/null 2>&1
git -c user.email=t@t -c user.name=t commit -qm fixture >/dev/null 2>&1

# Every normalisation, and the reason for it, lives in tests/scrub-reference.py.
scrub() {
  python3 "$HERE/scrub-reference.py" \
    "$WORK=WORK" "$RULES=RULES" "$BIG=BIG" "$TMPROOT=TMPROOT" \
    "$NEW_SESSION=SESSION" "$OLD_SESSION=SESSION"
}

FAILED=0

# stdout, stderr and the exit code are kept apart. Merged, an interleaving
# would vary between runs, and which stream a line came out on is itself part
# of the contract: a caller parses one and reads the other.
run_at() {
  local root="$1" label="$2"; shift 2
  local dir="$WORK/cap-$label"
  mkdir -p "$dir"
  "$TOOL" "$@" --root "$root" > "$dir/out" 2> "$dir/err"
  local rc=$?
  {
    printf -- '--- stdout\n'; scrub < "$dir/out"
    printf -- '--- stderr\n'; scrub < "$dir/err"
    printf -- '--- exit\n%s\n' "$rc"
  } > "$dir/all"
  if [ "$MODE" = record ]; then
    cp "$dir/all" "$OUT/$label.txt"
    printf '  recorded %s\n' "$label"
  elif [ ! -f "$OUT/$label.txt" ]; then
    printf '  FAIL %s has no recording\n' "$label"
    FAILED=1
  elif diff -q "$OUT/$label.txt" "$dir/all" >/dev/null 2>&1; then
    printf '  ok   %s\n' "$label"
  else
    printf '  FAIL %s\n' "$label"
    diff "$OUT/$label.txt" "$dir/all" | head -12
    FAILED=1
  fi
}

run() { run_at "$WORK" "$@"; }

printf '\nthe constructed fixture: every command, in the order a sweep uses them\n'
run census-absent status s
run census-new census s --rule "$RULES/catch.yml" --scope src --question "swallowed?"
SITE="$("$TOOL" next s --root "$WORK" | python3 -c "import json,sys; print(json.load(sys.stdin)['sites'][0]['site_id'])")"
run next-batch next s --limit 2
run verdict-one verdict s --site "$SITE" --verdict violation --note "returns an empty array" --method "read the source"
run show-site show s --site "$SITE"
run status-partial status s
run surfaces-empty surfaces s
run surfaces-propose surfaces s --propose
run recheck-none recheck s
run manifest manifest s
run report-partial report s
run list list
run census-rerun census s --rule "$RULES/catch.yml" --scope src

printf '\nthe refusals\n'
run bad-subcommand nosuchcommand
run bad-missing-arg verdict s --verdict pass --note "n" --method "m"
run bad-verdict-value verdict s --site "$SITE" --verdict maybe --note "n" --method "m"
run bad-verdict verdict s --site "$SITE" --verdict pass --note ""
run bad-question census s --rule "$RULES/catch.yml" --scope src --question "a different question"
run bad-sweep status nosuchsweep
run bad-scope census other --rule "$RULES/catch.yml" --scope nowhere --question q
run bad-site show s --site 0000000000000000

printf '\na real repository at a fixed commit\n'
if [ -d "$AXIOS_CACHE/.git" ] &&
   [ "$(git -C "$AXIOS_CACHE" rev-parse "$AXIOS_PIN^{commit}" 2>/dev/null)" = "$AXIOS_PIN" ]; then
  git clone --quiet --no-hardlinks "$AXIOS_CACHE" "$BIG" 2>/dev/null
  git -C "$BIG" checkout --quiet "$AXIOS_PIN"
  # Prove the pin rather than assume it. A cache that has drifted produces a
  # wrong comparison silently, which is worse than no comparison.
  if [ "$(git -C "$BIG" rev-parse HEAD)" = "$AXIOS_PIN" ]; then
    run_at "$BIG" axios-census census big --rule "$RULES_DIR/ts-catch-clause.yml" --scope lib --question "swallowed?"
    run_at "$BIG" axios-status status big
    run_at "$BIG" axios-manifest manifest big
    run_at "$BIG" axios-next next big --limit 3
    run_at "$BIG" axios-surfaces surfaces big --propose
    run_at "$BIG" axios-logical census defaults --rule "$RULES_DIR/ts-logical-default.yml" --scope lib --question "wanted?"
    run_at "$BIG" axios-js-census census js --rule "$RULES/js-catch.yml" --scope lib --question "swallowed?"
    run_at "$BIG" axios-js-status status js
    run_at "$BIG" axios-js-next next js --limit 5
    run_at "$BIG" axios-js-manifest manifest js
    run_at "$BIG" axios-js-report report js
    run_at "$BIG" axios-js-defaults census jsdef --rule "$RULES/js-default.yml" --scope lib --question "wanted?"
  else
    printf '  SKIP the clone is not at %s\n' "$AXIOS_PIN"
  fi
else
  printf '  SKIP no copy of the pinned fixture at %s\n' "$AXIOS_CACHE"
  printf '       set RESWEEP_FIXTURE_DIR, or clone axios and check out %s\n' "$AXIOS_PIN"
fi

if [ "$MODE" = record ]; then
  printf '\nrecorded %s outputs into tests/reference\n' "$(ls "$OUT" | wc -l | tr -d ' ')"
else
  [ "$FAILED" -eq 0 ] && printf '\nevery command matches the reference\n' || printf '\nsome commands differ from the reference\n'
  exit "$FAILED"
fi
