#!/usr/bin/env bash
set -euo pipefail
BIN="$HOME/.local/bin/clipfinch"
if [ -x "$BIN" ]; then
  "$BIN" --quit 2>/dev/null || true
  "$BIN" --uninstall-shortcuts || true
  "$BIN" --autostart off || true
fi
rm -f "$BIN" \
  "$HOME/.local/share/applications/clipfinch.desktop" \
  "$HOME/.local/share/icons/hicolor/scalable/apps/clipfinch.svg" \
  "$HOME/.local/share/icons/hicolor/512x512/apps/clipfinch.png"
echo "Removed. Your history is still in ~/.local/share/clipfinch (delete it if you want)."
