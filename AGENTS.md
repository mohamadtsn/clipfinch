# Repository Guidelines

## Project layout

Clipfinch is a Linux clipboard manager written in Rust 2021 with GTK4. The Cargo workspace has three crates:

- `crates/core`: clipboard models, classification, search, SQLite storage, settings and paste queue.
- `crates/linux`: session detection, X11 integration, paste injection, GNOME shortcuts and autostart.
- `crates/app`: GTK application, clipboard monitor, popup, settings, queue and tray. Shared styling lives in `src/style.css` and `src/palette-{light,dark}.css`.

`assets/` contains the desktop entry, icon and bundled fonts with their licenses. `scripts/` contains container launchers, installation helpers and tests. See `DESIGN.md` for the interface direction.

## Development and verification

Use Docker Compose for development and testing; do not install project dependencies on the host. From the repository root, after setting `CLIPFINCH_UID` and `CLIPFINCH_GID` to the host user and group IDs:

```bash
docker compose build dev desktop
docker compose run --rm dev cargo test --locked --workspace
docker compose run --rm dev cargo fmt --all -- --check
docker compose run --rm dev cargo clippy --locked --workspace --all-targets
docker compose run --rm dev cargo build --locked --release
docker compose run --rm dev cargo build --locked --release --no-default-features
docker compose run --rm dev python3 scripts/test_container.py
docker compose run --rm dev python3 scripts/test_desktop_commands.py
./scripts/container.sh --doctor
```

The desktop launcher requires a graphical session and session bus; see `README.md` for setup, optional AppArmor exception and operational limits. `scripts/deps-ubuntu.sh` and `scripts/install.sh` are the optional native installation path and modify the host. Do not run them without authorization.

## Conventions and tests

Use rustfmt defaults. Name Rust modules, functions and tests in `snake_case`, types in `PascalCase`, and constants in `SCREAMING_SNAKE_CASE`. Keep reusable logic in core, desktop integration in linux and GTK presentation in app. Reuse existing helpers and preserve error handling and saved-settings compatibility.

Place focused Rust regression tests beside their implementation. For UI changes, check light and dark appearance, keyboard access, scaling and mixed Persian/English content with synthetic data. Record session type, backend and any untested behavior; an isolated virtual display cannot prove real GNOME shortcuts or Wayland focus and paste behavior.

## Safety and review

Preserve secret-clipboard filtering. Never inspect, erase or migrate real clipboard history for tests or screenshots; use isolated state and synthetic data. Preserve unrelated GNOME bindings and do not change host shortcuts, autostart, input permissions or installed software without authorization.

Use short, descriptive commit subjects. PRs should describe behavior changes, relevant checks and session-specific limitations; include synthetic screenshots for visible UI changes.

@/home/mohamadtsn/.codex/RTK.md

<!-- codebase-memory-mcp:start -->
For structural codebase exploration, use the installed `codebase-memory` skill.
<!-- codebase-memory-mcp:end -->

Use `agent-desktop` to see and operate desktop apps. Before the first use, run `agent-desktop skills get desktop` and `agent-desktop skills get platform`, and follow them.
