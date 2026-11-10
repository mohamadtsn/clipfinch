#!/usr/bin/env bash
set -euo pipefail
export CLIPFINCH_SESSION_ACCESS=1
exec "$(dirname -- "${BASH_SOURCE[0]}")/container.sh" "$@"
