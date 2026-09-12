#!/usr/bin/env bash
# Install codesweep: the CLI on PATH, and the skill where Claude Code finds it.
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
#   CODESWEEP_BIN_DIR    where the CLI symlink goes
#   CODESWEEP_SKILL_DIR  where the skill symlink goes
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLI_SOURCE="$HERE/bin/codesweep"
SKILL_SOURCE="$HERE/skills/codesweep"

# Minimum Python that supports the syntax and stdlib this CLI uses.
MIN_PY_MAJOR=3
MIN_PY_MINOR=9

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
  if [ -n "${CODESWEEP_BIN_DIR:-}" ]; then
    printf '%s' "$CODESWEEP_BIN_DIR"
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
SKILL_DIR="${CODESWEEP_SKILL_DIR:-$HOME/.claude/skills}"
CLI_LINK="$BIN_DIR/codesweep"
SKILL_LINK="$SKILL_DIR/codesweep"

# ---------------------------------------------------------------- dependencies

check_dependencies() {
  echo "dependencies"

  if command -v ast-grep >/dev/null 2>&1; then
    ok "ast-grep $(ast-grep --version 2>/dev/null | awk '{print $2}')"
  elif command -v sg >/dev/null 2>&1; then
    ok "ast-grep (as sg)"
  else
    bad "ast-grep is not on PATH"
    if command -v brew >/dev/null 2>&1; then
      say "     install it with: brew install ast-grep"
    else
      say "     install it from: https://ast-grep.github.io/guide/quick-start.html"
    fi
    say "     codesweep will not run without it, and will not fall back to a"
    say "     text search, because a text search cannot give the guarantee it exists to provide."
  fi

  if command -v python3 >/dev/null 2>&1; then
    if python3 -c "import sys; sys.exit(0 if sys.version_info >= ($MIN_PY_MAJOR, $MIN_PY_MINOR) else 1)"; then
      ok "python3 $(python3 -c 'import sys; print(".".join(map(str, sys.version_info[:3])))')"
    else
      bad "python3 is older than $MIN_PY_MAJOR.$MIN_PY_MINOR"
    fi
  else
    bad "python3 is not on PATH"
  fi
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

  # Exercise the link this script made, not whatever `codesweep` PATH happens
  # to resolve to. Checking PATH first would let a pre-existing install
  # elsewhere report success for a link that never took.
  if [ -x "$CLI_LINK" ] && CODESWEEP_SESSION_ID="install-check" "$CLI_LINK" --help >/dev/null 2>&1; then
    ok "cli runs: $CLI_LINK"
  else
    bad "cli did not run at $CLI_LINK"
  fi

  # Reachability by bare name is separate, and is about PATH rather than the link.
  case ":$PATH:" in
    *":$BIN_DIR:"*)
      if command -v codesweep >/dev/null 2>&1; then
        local resolved
        resolved="$(command -v codesweep)"
        if [ "$resolved" = "$CLI_LINK" ]; then
          ok "on PATH as: codesweep"
        else
          say "     note: \`codesweep\` resolves to $resolved, an earlier install"
          say "     ahead of $BIN_DIR on PATH. Remove it or reorder PATH."
        fi
      else
        bad "$BIN_DIR is on PATH but codesweep is not resolvable"
      fi
      ;;
    *)
      bad "$BIN_DIR is not on PATH, so \`codesweep\` will not resolve"
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
    echo "removing codesweep"
    unlink_if_ours "$CLI_LINK" "$CLI_SOURCE" "cli"
    unlink_if_ours "$SKILL_LINK" "$SKILL_SOURCE" "skill"
    echo
    say "The repository itself is untouched. Session indexes under"
    say "\$TMPDIR/codesweep are removed by age, or delete that directory now."
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
