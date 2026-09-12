#!/usr/bin/env bash
# Stage the pinned fixture repository into the case workspace.
#
# Runs before the agent, as the operator, with the real filesystem reachable.
# The agent that follows is sandboxed to this working directory, so anything
# it must read has to be placed here first.
set -euo pipefail

PIN=8092aee7240220aa7913d163187b7fba8d9e5a51
UPSTREAM=https://github.com/axios/axios.git

# $HOME is remapped for the run, so the operator's cache is found through the
# user database rather than through the environment.
REAL_HOME=$(eval echo "~$(id -un)")
CACHE=${CODESWEEP_FIXTURE_DIR:-$REAL_HOME/.cache/github/axios/axios}

if [ -d "$CACHE/.git" ]; then
  # --no-hardlinks on purpose. A local clone links by default, and hard links
  # are what the harness refuses when it scans a directory.
  git clone --quiet --no-hardlinks "$CACHE" . 2>/dev/null
else
  git clone --quiet "$UPSTREAM" . 2>/dev/null || {
    echo "fixture unreachable: no local copy at $CACHE and no network" >&2
    exit 1
  }
fi

git checkout --quiet "$PIN" 2>/dev/null || {
  echo "fixture is not at the pinned commit $PIN" >&2
  exit 1
}

# Prove the pin rather than assume it. A cache that has drifted produces a
# wrong comparison silently, which is worse than no comparison.
HEAD_SHA=$(git rev-parse HEAD)
[ "$HEAD_SHA" = "$PIN" ] || { echo "expected $PIN, got $HEAD_SHA" >&2; exit 1; }
