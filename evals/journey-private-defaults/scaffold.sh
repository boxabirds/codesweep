#!/usr/bin/env bash
# Stage one directory of a private codebase into the case workspace, at a
# pinned commit.
#
# Pinned, because the graders name an exact number and that codebase changes
# daily. The suite that already tests against it says so in its own header:
# frozen counts over a moving tree fail for the wrong reason within a day and
# then get deleted. This case was written with a literal count and no pin, which
# was the same mistake one directory away, and this is the correction.
#
# The pin is verified rather than assumed. A checkout that has moved on produces
# a wrong comparison with nothing in the report revealing it, which is the exact
# failure the tool exists to remove.
set -euo pipefail

# 13 September 2026. The counts the graders name were measured at this commit:
# 123 places using || or ??, of which 50 are in .tsx files.
PIN=581cd0ee
SCOPE=workers/admin/src

# Resolved through the user database, not through HOME.
#
# Three things were tried and only this one works. $HOME is remapped for the
# run, so it points inside the sandbox. Exporting a variable from the runner
# does not reach here either: the harness scrubs the environment, and a run
# whose runner printed the path still saw the variable empty. Asking the user
# database for the real home directory does survive, which is what the axios
# fixture does and why it works.
REAL_HOME=$(eval echo "~$(id -un)")
REPO="${CEETRIX_REPO:-$REAL_HOME/expts/claude-backlog}"

if [ ! -d "$REPO/.git" ]; then
  echo "no private checkout at $REPO" >&2
  echo "this case measures against a codebase that is not part of this project." >&2
  echo "set CEETRIX_REPO to a checkout, or filter this case out." >&2
  exit 1
fi

if [ "$(git -C "$REPO" rev-parse --short "$PIN^{commit}" 2>/dev/null)" != "$PIN" ]; then
  echo "the checkout at $REPO does not contain $PIN" >&2
  echo "the graders name counts measured at that commit, so any other state" >&2
  echo "would be compared against the wrong numbers." >&2
  exit 1
fi

# From the commit, not the working tree. The working tree had fifty-four
# uncommitted changes when this was written; they happened not to touch this
# directory, which is luck rather than a property to rely on.
git -C "$REPO" archive "$PIN" "$SCOPE" | tar -x --strip-components=3 -C .

# A repository, so the walker behaves as it would in a real one.
git init -q . 2>/dev/null || true
git -c user.email=e@e -c user.name=e add -A >/dev/null 2>&1 || true
git -c user.email=e@e -c user.name=e commit -qm "staged at $PIN" >/dev/null 2>&1 || true
