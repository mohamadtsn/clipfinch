#!/usr/bin/env bash
set -euo pipefail
umask 077

repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
export CLIPFINCH_UID="$(id -u)"
export CLIPFINCH_GID="$(id -g)"
runtime_dir="${XDG_RUNTIME_DIR:-/run/user/$CLIPFINCH_UID}"
state_dir="${CLIPFINCH_STATE_DIR:-$repo_dir/.container}"
host_config="${XDG_CONFIG_HOME:-$HOME/.config}"
needs_bus=1

case "${1:---show}" in
  --version|-V|--help|-h|--doctor|--autostart) needs_bus=0 ;;
  --install-shortcuts|--uninstall-shortcuts) ;;
  *)
    if [[ -z "${DISPLAY:-}" && -z "${WAYLAND_DISPLAY:-}" ]]; then
      echo 'Clipfinch needs a desktop session; run this launcher from a desktop terminal.' >&2
      exit 1
    fi
    ;;
esac
if [[ "$needs_bus" == 1 && ! -S "$runtime_dir/bus" ]]; then
  echo 'Clipfinch needs the desktop session bus; run from the signed-in desktop session.' >&2
  exit 1
fi
if [[ "${1:-}" == --install-shortcuts || "${1:-}" == --uninstall-shortcuts ]]; then
  if [[ ! -d "$host_config/dconf" ]]; then
    echo 'Cannot safely register shortcuts without the host dconf settings directory.' >&2
    exit 1
  fi
fi

if ! docker image inspect clipfinch-desktop:local >/dev/null 2>&1; then
  echo "Build the desktop image first: docker compose -f \"$repo_dir/compose.yaml\" build desktop" >&2
  exit 1
fi

mkdir -p "$state_dir/data" "$state_dir/config" "$state_dir/cache"
compose=(docker compose --project-directory "$repo_dir" -f "$repo_dir/compose.yaml")
launcher="$repo_dir/scripts/container.sh"
if [[ "${CLIPFINCH_SESSION_ACCESS:-0}" == 1 ]]; then
  compose+=(-f "$repo_dir/compose.session.yaml")
  launcher="$repo_dir/scripts/container-session.sh"
fi
run=("${compose[@]}" run --rm --no-deps --pull never
  -e "CLIPFINCH_LAUNCHER=$launcher"
  -e "XDG_RUNTIME_DIR=$runtime_dir"
  -e XDG_DATA_HOME=/state/data -e XDG_CONFIG_HOME=/state/config -e XDG_CACHE_HOME=/state/cache
  -v "$state_dir/data:/state/data" -v "$state_dir/config:/state/config" -v "$state_dir/cache:/state/cache")

for name in DISPLAY WAYLAND_DISPLAY XAUTHORITY XDG_SESSION_TYPE XDG_CURRENT_DESKTOP GDK_BACKEND CLIPFINCH_BACKEND; do
  if [[ -n "${!name:-}" ]]; then run+=(-e "$name=${!name}"); fi
done

if [[ -S "$runtime_dir/bus" ]]; then
  run+=(-v "$runtime_dir/bus:$runtime_dir/bus"
    -e "DBUS_SESSION_BUS_ADDRESS=unix:path=$runtime_dir/bus")
fi
if [[ -d "$host_config/dconf" ]]; then
  run+=(-v "$host_config/dconf:/state/config/dconf:ro")
  mkdir -p "$runtime_dir/dconf"
  run+=(-v "$runtime_dir/dconf:$runtime_dir/dconf")
else
  # Keep GUI installation disabled rather than overwrite host settings from an empty view.
  run+=(-e CLIPFINCH_SHORTCUTS_UNAVAILABLE=1)
fi
if [[ -d "$runtime_dir/at-spi" ]]; then run+=(-v "$runtime_dir/at-spi:$runtime_dir/at-spi"); fi
if [[ -n "${WAYLAND_DISPLAY:-}" ]]; then
  if [[ "$WAYLAND_DISPLAY" == /* ]]; then wayland_socket="$WAYLAND_DISPLAY";
  else wayland_socket="$runtime_dir/$WAYLAND_DISPLAY"; fi
  if [[ -S "$wayland_socket" ]]; then run+=(-v "$wayland_socket:$wayland_socket"); fi
fi
if [[ "${DISPLAY:-}" == :* ]]; then
  display_number="${DISPLAY#:}"
  x_socket="/tmp/.X11-unix/X${display_number%%.*}"
  if [[ -S "$x_socket" ]]; then run+=(-v "$x_socket:$x_socket"); fi
fi
authority="${XAUTHORITY:-$HOME/.Xauthority}"
if [[ -f "$authority" ]]; then
  run+=(-e "XAUTHORITY=$authority" -v "$authority:$authority:ro")
fi
autostart_dir="${CLIPFINCH_AUTOSTART_DIR:-$host_config/autostart}"
mkdir -p "$autostart_dir"
run+=(-v "$autostart_dir:/state/config/autostart")

exec "${run[@]}" desktop "$@"
