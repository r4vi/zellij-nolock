# zellij-nolock

A Zellij plugin that prevents macOS screen lock while your terminal is active — the same way a video player does. It uses macOS `caffeinate` to create `PreventUserIdleDisplaySleep` IOKit power assertions, the identical mechanism apps like VLC and QuickTime use to keep your display awake.

## How it works

The plugin runs `caffeinate -d -t 120` via Zellij's `run_command` API. Only one process runs at a time — when it expires, the plugin immediately spawns the next one if still active.

**Auto mode** (default): activates when it detects pane activity (`PaneUpdate` events) and deactivates after 5 minutes of inactivity.

**Manual mode**: stays active until you toggle it off.

## Status indicator

| Icon | Meaning |
|------|---------|
| ☕ | Active — screen lock prevented |
| 💤 | Inactive — idle timeout reached |
| ⏳ | Waiting for permission grant |

## Building from source

### Prerequisites

Install [rustup](https://rustup.rs/) if you don't have it (Homebrew Rust alone won't work — it doesn't ship cross-compilation targets):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Add the WASM target:

```bash
rustup target add wasm32-wasip1
```

### Build

```bash
cargo build --release
```

The WASM binary is at `target/wasm32-wasip1/release/zellij-nolock.wasm`. The `.cargo/config.toml` in this repo already sets `wasm32-wasip1` as the default target, so no `--target` flag is needed.

Use the absolute path to this file when configuring Zellij (see Setup below).

## Setup

### Option A: Status bar indicator (recommended)

Embeds the plugin as a small widget in the tab bar row.

**1. Register the plugin alias** in `~/.config/zellij/config.kdl`:

```kdl
plugins {
    // ... existing aliases ...
    nolock location="file:/path/to/zellij-nolock.wasm"
}
```

**2. Create a layout** at `~/.config/zellij/layouts/default.kdl`:

```kdl
layout {
    default_tab_template {
        pane size=1 split_direction="vertical" {
            pane borderless=true {
                plugin location="zellij:tab-bar"
            }
            pane size=4 borderless=true {
                plugin location="nolock"
            }
        }
        children
        pane size=1 borderless=true {
            plugin location="zellij:status-bar"
        }
    }
}
```

**3. Restart Zellij.** The ☕ indicator appears at the right edge of the tab bar.

### Option B: Headless (no UI)

Runs silently in the background with no visible pane.

**1. Register the plugin alias** (same as above).

**2. Add to `load_plugins`** in `~/.config/zellij/config.kdl`:

```kdl
load_plugins {
    "file:/path/to/zellij-nolock.wasm"
}
```

**3. Restart Zellij.** The plugin manages `caffeinate` in the background. Use `pmset -g assertions` to verify it's working.

### Permissions

The plugin requires the `RunCommands` permission to execute `caffeinate`. On first load, Zellij should prompt you to grant it. The grant is cached at `~/Library/Caches/org.Zellij-Contributors.Zellij/permissions.kdl`.

If the prompt doesn't appear, create the file manually:

```kdl
"/path/to/zellij-nolock.wasm" {
    ReadApplicationState
    ChangeApplicationState
    RunCommands
}
```

## Configuration

Pass options via `--configuration` (CLI) or plugin block properties (layout):

| Option | Default | Description |
|--------|---------|-------------|
| `mode` | `auto` | `auto` or `manual` |
| `inactivity_timeout` | `300` | Seconds of inactivity before auto-deactivating |

Example in a layout:

```kdl
plugin location="nolock" {
    mode "manual"
    inactivity_timeout "600"
}
```

## Verify

```bash
pmset -g assertions | grep PreventUserIdleDisplaySleep
```

Should show `PreventUserIdleDisplaySleep    1` and a `caffeinate` entry with `-d -t 120` when active.
