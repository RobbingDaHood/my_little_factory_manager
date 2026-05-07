#!/usr/bin/env bash
# Launch the Zellij + nushell UI for My Little Factory Manager.
#
# Prerequisites:
#   - zellij (https://zellij.dev)
#   - nu (https://www.nushell.sh) — version 0.90+
#   - The game server running. Default URL: http://localhost:8000
#     Override with MLFM_BASE_URL=http://host:port ./start.sh
#
# Usage:
#   cd UIs/Zellij+nushell && ./start.sh

set -euo pipefail

dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$dir"

export MLFM_UI_DIR="$dir"
export MLFM_BASE_URL="${MLFM_BASE_URL:-http://localhost:8000}"

for cmd in zellij nu; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        echo "error: '$cmd' not found in PATH" >&2
        exit 1
    fi
done

# Make sure the shared state file exists with sane defaults.
mkdir -p state
if [[ ! -f state/ui.json ]]; then
    cat > state/ui.json <<'JSON'
{"library_filter":"","contracts_filter":"","refresh_ms":1000}
JSON
fi

echo "Connecting UI to $MLFM_BASE_URL"
exec zellij --layout "$dir/layout.kdl"
