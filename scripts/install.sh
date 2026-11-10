#!/usr/bin/env bash
# Builds Clipfinch and installs it for the current user (no sudo needed).
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release

BIN="$HOME/.local/bin/clipfinch"
install -Dm755 target/release/clipfinch "$BIN"
install -Dm644 assets/icon.png "$HOME/.local/share/icons/hicolor/512x512/apps/clipfinch.png"
rm -f "$HOME/.local/share/icons/hicolor/scalable/apps/clipfinch.svg"
mkdir -p "$HOME/.local/share/applications"
sed "s|^Exec=clipfinch|Exec=$BIN|" assets/clipfinch.desktop \
  > "$HOME/.local/share/applications/clipfinch.desktop"

gtk-update-icon-cache -q -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true

"$BIN" --install-shortcuts || echo "Could not install shortcuts automatically; open Settings in Clipfinch."
"$BIN" --autostart on || true

echo
echo "Installed to $BIN"
echo "Start it now with:  clipfinch --background &"
echo "Then press Super+Shift+V to open the history."
echo "Run 'clipfinch --doctor' to see what works on this system."
