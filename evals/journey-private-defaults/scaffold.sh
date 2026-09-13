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

# The path arrives in the environment, set by tests/run-evals.sh before the
# harness starts. It cannot be worked out here: HOME is remapped inside a run,
# and resolving the real one through the user database does not survive the
# sandbox either. Both were tried and both refused on a machine that has the
# checkout.
SOURCE="${CEETRIX_REPO:-}/workers/admin/src"

if [ -z "${CEETRIX_REPO:-}" ] || [ ! -d "$SOURCE" ]; then
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
