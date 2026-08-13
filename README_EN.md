# shell-switcher

[简体中文](README.md) | **English**

<div align="center">

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE) [![Language: Rust](https://img.shields.io/badge/Language-Rust-orange.svg)](https://www.rust-lang.org/) [![Stars](https://img.shields.io/github/stars/Shangshui0302/shell-switcher)](https://github.com/Shangshui0302/shell-switcher) [![Forks](https://img.shields.io/github/forks/Shangshui0302/shell-switcher)](https://github.com/Shangshui0302/shell-switcher)

</div>

A small tool to switch desktop shells (top panels) at runtime, for Hyprland / niri.

Multiple desktop shells compete for the `org.freedesktop.Notifications` DBus name and each draws its own top bar, so they cannot run at the same time. shell-switcher ensures only one shell runs at a time: it cleanly stops the current one, starts the target, and falls back to the default shell on failure — no manual `systemctl --user stop/start`.

> **About this project**: a quick AI-vibe script — single-file Rust, deliberately minimal, written for personal use, not a serious tool. It exists to be "good enough", so the code stays simple and trade-offs are direct (see [Limitations](#limits)). Fork freely, PRs welcome.

## Table of Contents

- [Features](#features)
- [How It Works](#how-it-works)
- [Installation](#install)
- [Configuration](#config)
- [Usage](#usage)
- [Adding a Shell (registration)](#add-shell)
- [Adding a Compositor (requires code changes)](#add-compositor)
- [Limitations](#limits)
- [License](#license)

## <a id="features"></a>Features

- **Switch with one command**: `shell-switcher set <name>` — internally stop-all → await exit → start target
- **Declarative shell registration**: add a shell by editing `config.toml` — no code changes, no recompilation
- **Safety guards**: refuses to switch outside Hyprland/niri sessions; falls back to the default shell on failure (config.toml `default`, first entry if unset)
- **Idempotent**: no-op when the target is already running and no other shell is
- **Startup recovery**: the `boot` entry reads the `current` marker to restore your last selection on compositor autostart
- **Shell completions**: bash / zsh / fish completions installed automatically with the Nix package; for cargo/source installs, place them manually (see [Installation](#install))
- **i18n**: Chinese / English chosen by system locale (locale containing `zh` → Chinese, otherwise → English)

## <a id="how-it-works"></a>How It Works

Each shell is a **systemd user service** (under the `graphical-session.target` context). `config.toml` declares a `name → service` mapping; the switcher only orchestrates start/stop — it does not manage shell installation.

What happens inside `set <name>`:

1. Detect the compositor (refuse outside Hyprland/niri)
2. Idempotent short-circuit: target already active and no other shell active → just update the marker
3. `systemctl --user stop` all shells, poll until all are inactive (**10s timeout**)
4. `systemctl --user start` the target, poll until active (**15s timeout**)
5. Write the `current` marker

Any failure falls back to the default shell. Shells stopped by SIGTERM (e.g. DMS, exit code 143) should set `SuccessExitStatus=143` in their service so systemd does not mark them as failed.

## <a id="install"></a>Installation

shell-switcher is an ordinary Rust binary, no Nix dependency. Requires a Rust toolchain (edition 2021); deps are only `serde` + `toml`. Works on any distro with a systemd user session + Hyprland/niri.

### Cargo install (recommended, any distro)

```bash
cargo install --path .         # install from a local checkout
# or install directly from git
cargo install --git https://github.com/Shangshui0302/shell-switcher
```

### Build from source

```bash
git clone https://github.com/Shangshui0302/shell-switcher
cd shell-switcher
cargo build --release
install -Dm755 target/release/shell-switcher ~/.local/bin/
```

Neither cargo install nor a source build places shell completions automatically; copy them manually if you want them:

```bash
install -Dm644 completions/shell-switcher.bash ~/.local/share/bash-completion/completions/shell-switcher
install -Dm644 completions/_shell-switcher ~/.local/share/zsh/site-functions/_shell-switcher
install -Dm644 completions/shell-switcher.fish ~/.local/share/fish/vendor_completions.d/shell-switcher.fish
```

After installing:

1. Write `~/.config/shell-switcher/config.toml` (format in [Configuration](#config)).
2. Declare the **systemd user services** for each shell using your distro's own mechanism (`/etc/systemd/user/` or `~/.config/systemd/user/`), and make the switchable shells mutually exclusive (`wantedBy` empty, **not auto-started** — the switcher starts/stops them).

Prerequisite: you must be inside a Hyprland/niri session, otherwise `set`/`current` refuse to run.

### Nix flake (optional, for NixOS / Nix users)

The flake packages via `rustPlatform.buildRustPackage`, artifacts in `packages.default`:

```bash
nix build .#default          # build the binary (result/bin/shell-switcher)
nix develop                  # dev environment (rustc/cargo/rust-analyzer/clippy)
```

Use it as a flake input in NixOS/Home Manager:

```nix
# flake.nix
inputs.shell-switcher = {
  url = "github:Shangshui0302/shell-switcher";
  inputs.nixpkgs.follows = "nixpkgs";
};

# in some module
home.packages = [
  inputs.shell-switcher.packages.${pkgs.system}.default
];
```

## <a id="config"></a>Configuration

Config file: `~/.config/shell-switcher/config.toml`. If missing or unparsable, commands report a clear error (showing the config path) rather than silently assuming built-ins.

```toml
default = "noctalia"        # default shell (optional, first [[shell]] if unset)

[[shell]]
name = "noctalia"
service = "noctalia.service"

[[shell]]
name = "dms"
service = "dms.service"
```

Fields:

| Field | Description |
|-------|-------------|
| `default` | Default shell name (optional): used when `boot` has no `current` marker or a switch fails and falls back; first `[[shell]]` if unset |
| `name` | Shell identifier inside the switcher (what `set <name>` takes) |
| `service` | The corresponding systemd user service name (must end in `.service`) |

`current` marker file: `~/.config/shell-switcher/current` (written by `set`, read by `boot`; content is the current shell name).

## <a id="usage"></a>Usage

```bash
shell-switcher list               # list shells registered in config.toml
shell-switcher current            # show the currently active shell (none if none)
shell-switcher set <name>         # switch to a shell
shell-switcher boot               # read the current marker and start that shell (shell-starter entry)
shell-switcher help               # help
```

Typical scenarios:

```bash
# switch from the default shell to another
shell-switcher set dms

# switch back
shell-switcher set noctalia

# as part of compositor autostart / a desktop startup script:
# only start a non-default shell when the marker says so (the default shell
# is started automatically by systemd via WantedBy)
shell-switcher boot
```

## <a id="add-shell"></a>Adding a Shell (registration)

Registering a shell is **fully declarative** — add one `[[shell]]` entry, no code changes:

1. Make sure the shell's systemd user service exists: put a unit file in `/etc/systemd/user/` (system-level) or `~/.config/systemd/user/` (user-level), then `systemctl --user daemon-reload`; NixOS/Home Manager users can also use `systemd.user.services`.
2. Add the mapping to `config.toml`:

```toml
[[shell]]
name = "my-shell"
service = "my-shell.service"
```

3. `shell-switcher list` to confirm it shows up, `shell-switcher set my-shell` to switch.

Practice notes:

- Shells must be **mutually exclusive** (only one running at a time): switchable shells should have an empty `wantedBy` — **not auto-started**, the switcher starts/stops them; the default shell uses `WantedBy=graphical-session.target` to auto-start.
- Services should exit cleanly: set `KillMode=control-group` (kills QML child processes too), and `SuccessExitStatus=143` when needed (so SIGTERM-stopped services are not marked failed).

## <a id="add-compositor"></a>Adding a Compositor (requires code changes)

Compositor detection is currently **hardcoded**, not configurable. `detect_compositor()` only recognizes two environment variables:

| Compositor | Env var |
|------------|---------|
| Hyprland | `HYPRLAND_INSTANCE_SIGNATURE` |
| niri | `NIRI_SOCKET` |

To support a new compositor (e.g. Sway via `SWAYSOCK`), add a branch in `detect_compositor()` in `src/main.rs`:

```rust
fn detect_compositor() -> Option<&'static str> {
    if env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
        Some("hyprland")
    } else if env::var("NIRI_SOCKET").is_ok() {
        Some("niri")
    } else if env::var("SWAYSOCK").is_ok() {
        Some("sway")
    } else {
        None
    }
}
```

If you later want the compositor list to be configurable too, the detection conditions could move into `config.toml` (not implemented yet).

## <a id="limits"></a>Limitations

- Only Hyprland / niri (compositor detection hardcoded, see above)
- Assumes all shell services live under the `graphical-session.target` context
- Stop/start timeouts are fixed (10s / 15s), not configurable
- Switching is "stop all → start target", not "start target before stopping the old one", so there is a brief gap during a switch

## <a id="license"></a>License

MIT License — see [LICENSE](LICENSE).
