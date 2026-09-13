#!/usr/bin/env bash
# Install resweep: the CLI on PATH, and the skill where Claude Code finds it.
#
# Idempotent. Re-running reports what is already in place and changes only what
# is not. Nothing is overwritten without saying so, and a real file sitting
# where a symlink should go is an error rather than something to clobber.
#
#   ./install.sh              install, or bring an existing install up to date
#   ./install.sh --uninstall  remove the links this script created
#   ./install.sh --check      report state and exit, changing nothing
#
# Overrides, mostly for tests:
#   RESWEEP_BIN_DIR    where the CLI symlink goes
#   RESWEEP_SKILL_DIR  where the skill symlink goes
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLI_SOURCE="$HERE/bin/resweep"
SKILL_SOURCE="$HERE/skills/resweep"

# Where cargo puts the binary this script installs.
BUILT="$HERE/target/release/resweep"

CHANGED=0
PROBLEMS=0

say()  { printf '  %s\n' "$*"; }
ok()   { printf '  \033[32mok\033[0m    %s\n' "$*"; }
did()  { printf '  \033[33mdone\033[0m  %s\n' "$*"; CHANGED=$((CHANGED + 1)); }
bad()  { printf '  \033[31mfail\033[0m  %s\n' "$*"; PROBLEMS=$((PROBLEMS + 1)); }

MODE=install
case "${1:-}" in
  --uninstall) MODE=uninstall ;;
  --check)     MODE=check ;;
  --help|-h)   sed -n '2,16p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
  "")          ;;
  *)           echo "unknown option: $1 (try --help)" >&2; exit 2 ;;
esac

# ---------------------------------------------------------------- where things go

pick_bin_dir() {
  if [ -n "${RESWEEP_BIN_DIR:-}" ]; then
    printf '%s' "$RESWEEP_BIN_DIR"
    return
  fi
  # Prefer a directory already on PATH and writable, so the install works
  # without the user editing a shell profile. ~/.local/bin is the fallback
  # because it is the conventional place, even when it is not yet on PATH.
  local candidate
  for candidate in "$HOME/.local/bin" /opt/homebrew/bin /usr/local/bin "$HOME/bin"; do
    case ":$PATH:" in
      *":$candidate:"*) if [ -d "$candidate" ] && [ -w "$candidate" ]; then
                          printf '%s' "$candidate"; return
                        fi ;;
    esac
  done
  printf '%s' "$HOME/.local/bin"
}

BIN_DIR="$(pick_bin_dir)"
SKILL_DIR="${RESWEEP_SKILL_DIR:-$HOME/.claude/skills}"
CLI_LINK="$BIN_DIR/resweep"
SKILL_LINK="$SKILL_DIR/resweep"

# ---------------------------------------------------------------- dependencies

# The tool is built here rather than downloaded, because a plugin cannot carry
# a build for every machine. Measured, not assumed: neither the plugin manifest
# nor the marketplace entry has any field naming an operating system or a
# processor, and a manifest declaring a platform this machine is not still
# installs and still runs. Shipping every build instead would be over two
# hundred megabytes in a repository that is cloned on every install, because the
# language grammars are almost the whole binary.
#
# So one prerequisite, a Rust toolchain, and nothing at all at run time: the
# matching engine and the storage engine are compiled in.
check_dependencies() {
  echo "dependencies"

  if command -v cargo >/dev/null 2>&1; then
    ok "cargo $(cargo --version 2>/dev/null | awk '{print $2}')"
  else
    bad "cargo is not on PATH, so resweep cannot be built"
    say "     install a Rust toolchain from https://rustup.rs"
    say "     Nothing else is needed. The matching engine and the storage engine"
    say "     are compiled into the binary, so there is no second program to"
    say "     install and no version of one to be wrong."
  fi
}

build() {
  echo "build"
  if ! command -v cargo >/dev/null 2>&1; then
    bad "skipped: no cargo"
    return
  fi
  # Quiet unless it fails. A successful build has nothing to say and a failed
  # one has everything, so the output is worth seeing only in the second case.
  local log
  log="$(mktemp)"
  if (cd "$HERE" && cargo build -p resweep --release --quiet) >"$log" 2>&1; then
    ok "built $BUILT ($(du -h "$BUILT" 2>/dev/null | awk '{print $1}'))"
  else
    bad "the build failed"
    sed 's/^/       /' "$log"
  fi
  rm -f "$log"
}

# ---------------------------------------------------------------- linking

# Create or refresh one symlink. Reports which of the three cases applied:
# already correct, repointed, or created. Refuses to replace anything that is
# not a symlink, because that is someone else's file.
link() {
  local source="$1" target="$2" label="$3"

  if [ ! -e "$source" ]; then
    bad "$label: source missing at $source"
    return
  fi

  if [ -L "$target" ]; then
    local current
    current="$(readlink "$target")"
    if [ "$current" = "$source" ]; then
      ok "$label already linked: $target"
      return
    fi
    ln -sfn "$source" "$target" && did "$label repointed: $target (was $current)"
    return
  fi

  if [ -e "$target" ]; then
    bad "$label: $target exists and is not a symlink. Move it aside and re-run."
    return
  fi

  mkdir -p "$(dirname "$target")" || { bad "$label: cannot create $(dirname "$target")"; return; }
  ln -sfn "$source" "$target" && did "$label linked: $target"
}

unlink_if_ours() {
  local target="$1" source="$2" label="$3"
  if [ ! -L "$target" ]; then
    if [ -e "$target" ]; then
      say "$label: $target is not a symlink, leaving it alone"
    else
      ok "$label already absent"
    fi
    return
  fi
  if [ "$(readlink "$target")" != "$source" ]; then
    say "$label: $target points elsewhere, leaving it alone"
    return
  fi
  rm "$target" && did "$label removed: $target"
}

# ---------------------------------------------------------------- verification

verify() {
  echo "verification"

  if [ ! -x "$CLI_SOURCE" ]; then
    chmod +x "$CLI_SOURCE" 2>/dev/null && did "made $CLI_SOURCE executable"
  fi

  # Exercise the link this script made, not whatever `resweep` PATH happens
  # to resolve to. Checking PATH first would let a pre-existing install
  # elsewhere report success for a link that never took.
  if [ -x "$CLI_LINK" ] && RESWEEP_SESSION_ID="install-check" "$CLI_LINK" --help >/dev/null 2>&1; then
    ok "cli runs: $CLI_LINK"
  else
    bad "cli did not run at $CLI_LINK"
  fi

  # Reachability by bare name is separate, and is about PATH rather than the link.
  case ":$PATH:" in
    *":$BIN_DIR:"*)
      if command -v resweep >/dev/null 2>&1; then
        local resolved
        resolved="$(command -v resweep)"
        if [ "$resolved" = "$CLI_LINK" ]; then
          ok "on PATH as: resweep"
        else
          say "     note: \`resweep\` resolves to $resolved, an earlier install"
          say "     ahead of $BIN_DIR on PATH. Remove it or reorder PATH."
        fi
      else
        bad "$BIN_DIR is on PATH but resweep is not resolvable"
      fi
      ;;
    *)
      bad "$BIN_DIR is not on PATH, so \`resweep\` will not resolve"
      say "     add it:  echo 'export PATH=\"$BIN_DIR:\$PATH\"' >> ~/.zshrc && exec zsh"
      ;;
  esac

  if [ -L "$SKILL_LINK" ] && [ -f "$SKILL_LINK/SKILL.md" ]; then
    ok "skill readable at $SKILL_LINK"
  else
    bad "skill not readable at $SKILL_LINK"
  fi
}

# ---------------------------------------------------------------- run

echo
case "$MODE" in
  uninstall)
    echo "removing resweep"
    unlink_if_ours "$CLI_LINK" "$CLI_SOURCE" "cli"
    unlink_if_ours "$SKILL_LINK" "$SKILL_SOURCE" "skill"
    echo
    say "The repository itself is untouched. Session indexes under"
    say "\$TMPDIR/resweep are removed by age, or delete that directory now."
    ;;

  check)
    check_dependencies
    echo
    echo "links"
    [ -L "$CLI_LINK" ]   && ok "cli   $CLI_LINK -> $(readlink "$CLI_LINK")"     || bad "cli   not linked at $CLI_LINK"
    [ -L "$SKILL_LINK" ] && ok "skill $SKILL_LINK -> $(readlink "$SKILL_LINK")" || bad "skill not linked at $SKILL_LINK"
    echo
    verify
    ;;

  install)
    check_dependencies
    build
    echo
    echo "links"
    link "$CLI_SOURCE" "$CLI_LINK" "cli"
    link "$SKILL_SOURCE" "$SKILL_LINK" "skill"
    echo
    verify
    echo
    if [ "$CHANGED" -eq 0 ]; then
      say "Nothing to do; already installed."
    fi
    if [ "$PROBLEMS" -eq 0 ]; then
      say "Ask an agent to find every instance of something and it will use this."
      say "Sessions already running need a restart to see the skill."
    fi
    ;;
esac

echo
if [ "$PROBLEMS" -gt 0 ]; then
  printf '  %s problem(s). Nothing above was guessed at; fix and re-run.\n\n' "$PROBLEMS"
  exit 1
fi
exit 0
