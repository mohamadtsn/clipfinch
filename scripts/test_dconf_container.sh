#!/usr/bin/env bash
# Verifies real dconf reads/writes across containers without the user's session bus.
set -euo pipefail
repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "$repo_dir/.container"
test_root="$(mktemp -d "$repo_dir/.container/dconf-test.XXXXXX")"
export CLIPFINCH_UID="$(id -u)" CLIPFINCH_GID="$(id -g)"
export XDG_CONFIG_HOME="$test_root/host-config"
export XDG_RUNTIME_DIR="$test_root/runtime"
export CLIPFINCH_STATE_DIR="$test_root/state"
export CLIPFINCH_AUTOSTART_DIR="$test_root/autostart"
export DBUS_SESSION_BUS_ADDRESS="unix:path=$XDG_RUNTIME_DIR/bus"
unset CLIPFINCH_SESSION_ACCESS
mkdir -p "$XDG_CONFIG_HOME" "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
bus_container=""
cleanup() {
  if [[ -n "$bus_container" ]]; then docker stop "$bus_container" >/dev/null; fi
  rm -rf -- "$test_root"
}
trap cleanup EXIT
bus_container="$(docker compose --project-directory "$repo_dir" -f "$repo_dir/compose.yaml" \
  run -d --rm --no-deps -v "$test_root:$test_root" \
  -e XDG_CONFIG_HOME -e XDG_RUNTIME_DIR -e DBUS_SESSION_BUS_ADDRESS dev \
  dbus-daemon --session --nofork --address="$DBUS_SESSION_BUS_ADDRESS")"
for _ in {1..100}; do
  if [[ -S "$XDG_RUNTIME_DIR/bus" ]]; then break; fi
  sleep 0.05
done
[[ -S "$XDG_RUNTIME_DIR/bus" ]]
schema=org.gnome.settings-daemon.plugins.media-keys
unrelated=/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/
docker exec "$bus_container" gsettings set "$schema" custom-keybindings "['$unrelated']"
"$repo_dir/scripts/container.sh" --install-shortcuts
first="$(docker exec "$bus_container" gsettings get "$schema" custom-keybindings)"
[[ "$first" == *"'$unrelated'"* && "$first" == *"clipfinch-popup/"* \
   && "$first" == *"clipfinch-queue/"* && "$first" == *"clipfinch-queue-paste/"* ]]
"$repo_dir/scripts/container.sh" --install-shortcuts
[[ "$(docker exec "$bus_container" gsettings get "$schema" custom-keybindings)" == "$first" ]]
"$repo_dir/scripts/container.sh" --uninstall-shortcuts
[[ "$(docker exec "$bus_container" gsettings get "$schema" custom-keybindings)" == "['$unrelated']" ]]
echo 'PASS: private dconf preserves unrelated bindings, repeated install is idempotent, removal is scoped.'
