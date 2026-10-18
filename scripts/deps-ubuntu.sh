#!/usr/bin/env bash
# Installs build dependencies on Ubuntu 24.04 or newer.
set -euo pipefail

sudo apt update
sudo apt install -y build-essential pkg-config \
  libgtk-4-dev libsqlite3-dev libdbus-1-dev libx11-dev libxtst-dev libxkbcommon-dev

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo not found: installing Ubuntu's Rust packages"
  sudo apt install -y cargo rustc
fi
cargo --version
