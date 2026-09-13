#!/usr/bin/env bash
# The harness requires a scaffold path inside the case directory, so this is a
# shim. One implementation lives in evals/fixture and every case reaches it
# from here, rather than five copies drifting apart.
set -euo pipefail
exec "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/fixture/axios.sh"
