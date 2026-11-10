![Clipfinch: clipboard history for Linux](assets/clipfinch-cover.png)

# Clipfinch

Clipfinch keeps your clipboard history close at hand on Linux. Search previous
copies, organize frequently used clips into pinboards, and paste from the keyboard.
It is a Rust and GTK4 application built for Ubuntu and GNOME.

> **Status:** Early alpha. Tested on Ubuntu 26.04; desktop integration and paste
> behavior depend on the session and available tools. See [X11 and Wayland](#x11-and-wayland).

## What it does

- Keeps text, links, images, files and colors in a searchable history, retaining
  clipboard formats for rich text, images and file copies.
- Organizes reusable clips into pinboards. Drag a clip to a tab or use its action
  menu; pinned clips do not expire.
- Previews, renames, opens and deletes clips, and pastes text without formatting.
- Queues multiple clips for pasting one at a time.
- Excludes clipboard content marked secret by password managers and supports
  per-app exclusions on X11.
- Stores history locally in SQLite, with configurable retention, a tray icon and
  autostart support.

Search understands clip kinds: `mail link slides` finds links matching both
`mail` and `slides` in the clip text, source app or title. Use quotes to search
for a kind word literally. Supported kinds are `text`, `link`, `image`, `file`
and `color`.

## Get started

### Install a release

Download a package from the repository's GitHub Releases page and open it with
your software installer. Choose `.deb` for Ubuntu/Debian-based systems or `.rpm`
for Fedora-based systems, and choose `amd64`/`x86_64` for most Intel and AMD
PCs or `arm64`/`aarch64` for ARM systems. Packages are built on Ubuntu 24.04 and
Fedora 43 respectively; other versions and distributions may have incompatible
libraries. Configure shortcuts and autostart in Settings after installation.
Shortcut registration currently targets GNOME; clipboard capture and automatic
paste also depend on the desktop session and backend.

For other distributions, the release also includes a `.tar.gz` containing the
compiled executable. Extract it and run `clipfinch`; it requires compatible
system libraries including glibc 2.39 and GTK 4.12 or newer; it does not install
a desktop entry. Keep the executable at the same path after enabling shortcuts
or autostart.

### Docker (Ubuntu/GNOME)

Use the existing Docker Engine and Compose plugin. Rust, Cargo, GTK headers,
formatting, lint and test tools stay inside the image; no host dependency
installation is needed.

```bash
export CLIPFINCH_UID="$(id -u)" CLIPFINCH_GID="$(id -g)"
docker compose build dev desktop
docker compose run --rm dev cargo test --locked -p clipfinch-core -p clipfinch-linux
docker compose run --rm dev cargo fmt --all -- --check
docker compose run --rm dev cargo clippy --locked --workspace --all-targets
docker compose run --rm dev cargo build --locked --release
docker compose run --rm dev cargo build --locked --release --no-default-features
./scripts/container.sh --show
```

Run the launcher from your desktop session, with its display/authentication and
D-Bus environment. Rebuild `desktop` after source changes. Cargo and target
caches use Docker volumes; source changes are visible immediately in `dev`.
There is no separate database service because SQLite is embedded.

The desktop launcher mounts individual display/session sockets, the autostart
directory and a read-only view of the host dconf directory, never your whole home
or existing clipboard-history directory. The shared dconf view keeps GNOME
shortcut reads consistent with the host writer so unrelated bindings are preserved. Its
history/settings live in the ignored `.container/data/clipfinch` directory;
`CLIPFINCH_STATE_DIR` selects a different directory. Session D-Bus access lets
the app interact with GNOME, including updating keybindings when requested.
It is not an isolated desktop or permission to read unrelated clipboard data.

Ubuntu's AppArmor policy may deny a container access to the session bus even
when its socket is mounted. For trusted desktop code only, the explicit opt-in
`./scripts/container-session.sh --show` adds
`compose.session.yaml`, disabling AppArmor confinement for the desktop container.
The development container stays confined, capabilities remain dropped, and no
host policy is changed. Use the same opt-in for subsequent desktop commands
when this policy applies; normal builds/tests do not need it. Registering from
this explicit launcher preserves that choice in the saved shortcut/autostart
command, rather than relying on a terminal's temporary environment variable.

For synthetic checks, use a dedicated state directory, create
`data/clipfinch/settings.json` containing `{"paused":true}` before starting,
and set `CLIPFINCH_AUTOSTART_DIR` to a test directory. Stop the normal instance
first: commands share the application ID even when data directories differ.
Never use real clipboard history in screenshots.

```bash
./scripts/container.sh --doctor
./scripts/container.sh --install-shortcuts
./scripts/container.sh --autostart on
```

Shortcut and autostart commands must target the absolute host launcher path,
provided inside the container as `CLIPFINCH_LAUNCHER`. Keep the checkout at that
path; re-register after moving it. The daemon and desktop session must be
available at login. Enabling autostart writes a desktop entry in your host
autostart directory; building images does not enable it.

The container does not gain input devices or install a host paste-injection
daemon. On Wayland, copying a selected item remains available even when
automatic paste is unavailable. A virtual display can check rendering but
cannot validate real GNOME shortcuts or compositor focus behavior.

### Native installation (Ubuntu 24.04+)

This path installs host build dependencies. It is not needed for Docker usage.

```bash
./scripts/deps-ubuntu.sh        # build dependencies (+ cargo if missing)
./scripts/install.sh            # build, install to ~/.local, register shortcuts
clipfinch --background &        # or log out and in (autostart is enabled)
```

## Keyboard shortcuts

Default GNOME custom keybindings can be edited in Settings:

| Shortcut | Action |
|---|---|
| Super+Shift+V | open / close the history popup |
| Super+Shift+C | start / stop the Paste Queue |
| Super+Shift+X | paste the next queued item |

Check what works on your machine: `clipfinch --doctor`.

### In the popup

| Key | Action |
|---|---|
| Up / Down, PgUp / PgDn | move selection |
| Enter / Shift+Enter | paste / paste as plain text |
| Ctrl+1 … Ctrl+9 | paste the first nine cards |
| Space | preview (when the search box is empty) |
| Tab, Shift+Tab, Left / Right | switch tab |
| Ctrl+C | copy without pasting |
| Ctrl+O / Ctrl+R | open link or file / rename |
| Ctrl+Backspace | delete (or remove from pinboard) |
| Ctrl+N | new pinboard |
| Ctrl+, | settings |
| Esc | clear search, then close |

## X11 and Wayland

Wayland deliberately blocks several things a clipboard manager would like to do.
Clipfinch handles each as follows:

| Feature | X11 session | Wayland session (Ubuntu default) |
|---|---|---|
| Background clipboard capture | yes | yes, by running through XWayland (`GDK_BACKEND=x11,wayland`) |
| Global shortcut | GNOME keybinding | GNOME keybinding (`clipfinch --toggle`) |
| Popup position | near the mouse pointer | chosen by the compositor |
| Opens at the text cursor | not implemented | impossible for normal apps |
| Automatic Ctrl+V | XTest | `ydotool` (optional, see below) |
| Source app name / excluded apps | yes | no |

Without automatic paste, selecting a clip still puts it on the clipboard; you press
Ctrl+V yourself.

### Optional native-only setup: automatic paste on Wayland with ydotool

```bash
sudo apt install ydotool
echo 'KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"' \
  | sudo tee /etc/udev/rules.d/80-uinput.rules
sudo usermod -aG input "$USER"      # log out and in afterwards
ydotoold &                          # start the daemon (add it to your autostart)
```

Set `CLIPFINCH_BACKEND=wayland` to run natively on Wayland instead of through
XWayland (background capture may then stop working while Clipfinch is not focused).

## Development and packaging

```bash
cargo build --release
cargo test -p clipfinch-core -p clipfinch-linux
cargo run -p clipfinch -- --background
cargo run -p clipfinch -- --toggle
cargo build --release --no-default-features    # without the tray icon
cargo install cargo-deb && cargo deb -p clipfinch
```

Publishing a GitHub Release builds Debian packages, Fedora RPM packages and
binary archives for x86-64 and ARM64. The package version comes from `Cargo.toml`;
update it before tagging a release. Pre-releases use the same workflow.

If `rustup` complains that Rust is already installed from apt, skip rustup: the apt
toolchain is fine (`sudo apt install cargo`).

## Project layout

```
crates/core    data model, classification, search, SQLite store, settings, paste queue
crates/linux   session detection, X11 helpers, paste injection, GNOME shortcuts, autostart
crates/app     GTK4 app: clipboard monitor, popup, queue, settings, tray
assets/        desktop file, icon and bundled fonts
scripts/       container launchers, installation helpers and tests
```

## Differences from Copywren

- Paste Queue is driven by its own shortcut (`--queue-paste`) rather than by
  intercepting Ctrl+V, which Wayland does not allow.
- No "hide from screen sharing" option (no equivalent on Linux).
- The popup does not follow the text cursor (see the Wayland table).

## About this project

Clipfinch was written with the help of [Claude](https://claude.ai) (Anthropic).
It is inspired by [Copywren](https://github.com/sonatina-dream/copywren), a clipboard
manager for macOS, because I wanted something like it on Ubuntu. It shares no code
with Copywren and is not affiliated with it. Credit for the original idea goes to its author.

## License

MIT, see [LICENSE](LICENSE).
