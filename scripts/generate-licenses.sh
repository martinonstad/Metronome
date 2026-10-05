#!/usr/bin/env bash
# Writes (or with --check, verifies) the license notices that the About screen shows.
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck source=scripts/env.sh
. scripts/env.sh
exec python3 scripts/generate-licenses.py "$@"
