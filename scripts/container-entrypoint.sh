#!/bin/sh
set -eu
case "${1:---show}" in
  --version|-V|--help|-h|--doctor|--autostart) ;;
  *)
    if ! gdbus call --session --dest org.freedesktop.DBus \
      --object-path /org/freedesktop/DBus --method org.freedesktop.DBus.GetId >/dev/null 2>&1; then
      echo 'Cannot connect to the desktop session bus. Check session sockets and the documented AppArmor opt-in.' >&2
      exit 1
    fi
    ;;
esac
exec /usr/local/bin/clipfinch "$@"
