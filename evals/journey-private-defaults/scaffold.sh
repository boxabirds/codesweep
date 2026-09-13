#!/usr/bin/env bash
# Stage a private codebase into the case workspace.
#
# This repository is not public and is not in this project. The case runs only
# on a machine that already has a checkout, and is skipped everywhere else by
# staging nothing, which makes the case fail loudly rather than quietly measure
# an empty directory.
#
# Only one directory is copied, not the whole monorepo: the question is about
# that directory and copying the rest would cost minutes per run for files no
# question touches.
set -euo pipefail

# Resolved through the user database, not through HOME.
#
# Three things were tried and only this one works. $HOME is remapped for the
# run, so it points inside the sandbox. Exporting a variable from the runner
# does not reach here either: the harness scrubs the environment, and a run
# whose runner printed the path still saw the variable empty. Asking the user
# database for the real home directory does survive, which is what the axios
# fixture does and why it works.
REAL_HOME=$(eval echo "~$(id -un)")
SOURCE="${CEETRIX_REPO:-$REAL_HOME/expts/claude-backlog}/workers/admin/src"

if [ ! -d "$SOURCE" ]; then
  echo "no private checkout at $SOURCE" >&2
  echo "this case measures against a codebase that is not part of this project." >&2
  echo "set CEETRIX_REPO to a checkout, or filter this case out." >&2
  exit 1
fi

# Copied rather than linked, and the copy is what the agent sees. Nothing the
# agent does can reach the original.
cp -R "$SOURCE/." .
# A repository, so the walker behaves as it would in a real one.
git init -q . 2>/dev/null || true
git -c user.email=e@e -c user.name=e add -A >/dev/null 2>&1 || true
git -c user.email=e@e -c user.name=e commit -qm "staged" >/dev/null 2>&1 || true
