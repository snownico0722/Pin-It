# PinIt in Rust (Slint) — release 4.0.0

A pure-Rust port of PinIt: no Qt, no webview, no JavaScript. The UI is
[Slint](https://slint.dev) and every Windows call goes through the
[`windows`](https://crates.io/crates/windows) crate. It reads and writes the
same `pinned.json` as the Qt build, so pins and settings carry over.

Slint draws with its software renderer. For one small window it looks the
same as OpenGL and keeps the GPU driver out of the process. Measured on
Windows 10 with the window open:

| | Rust + Slint | Qt 6 build (2.2.0) |
|---|---|---|
| Files shipped | 1 exe, 9.5 MB | 22 files, 35.8 MB |
| Private memory | about 6 MB | about 11 MB |
| Working set | about 22 MB | about 34 MB |

## Build and run

```bash
cargo run              # debug
cargo build --release  # single PinIt exe in target/release
cargo test
```

The version lives only in `Cargo.toml`: `build.rs` writes it into the exe's
file properties and `installer/PinIt.iss` reads it from there. CI
(`.github/workflows/build.yml`) runs the tests and builds the installer and
the portable ZIP from `target/release/pinit.exe`.

PinIt's frameless title area can be dragged to move the window. Click the
top-left logo/name to collapse PinIt to an always-on-top rounded mini window;
drag it to move or click to restore the previous size and **previous manual pin
choice**. The titlebar pin controls PinIt's long-term topmost preference, with
distinct on/off icons. By default, PinIt also temporarily stays on top while
focused (Settings → **Always on top while PinIt has focus**), then returns to
the manual preference when focus leaves. A hotkey/tray wake raises PinIt before
requesting focus so it can appear above another topmost window.

Two additional Settings control PinIt's own Z-order while the titlebar pin is
on or mini mode is active. **Keep PinIt above other topmost windows** (on by
default) re-raises it when the Windows foreground changes, without activating
it or stealing the focused app's keyboard input. **Aggressive mode** (off by
default) adds a 500-ms check, only raising when a visible overlapping topmost
window is above PinIt. Aggressive mode is inactive when absolute topmost is
off, but its saved choice is retained. Both stop raising when PinIt is
minimized or hidden. Focus-only temporary pinning never uses these policies.

The main action is split in two: **Pin a window…** opens the existing picker;
**Drag to pin** can be dragged onto another visible application window and
released to pin it. Dragging never unpins an already pinned window.

Managed rows have a separate pin switch: turning it off keeps the window in
the list, with its opacity and click-through settings. **Unpin all** turns
off all switches, including waiting entries, without removing them. The
row's **×** unpins the window, restores its original opacity/click-through,
and removes it from management. Both active and inactive entries survive
restarts; inactive entries are restored without turning topmost back on.

The top-right minus minimizes PinIt. The close button hides it to the tray by
default; Settings → **Exit PinIt instead of hiding to tray** changes that button
(and Windows window-close requests) into a real exit. Exiting matches tray Quit:
managed windows are restored and forgotten, and the tray icon is removed.
Click-to-compact is limited to the left logo/name; the middle of the titlebar
still drags the window without toggling compact mode.
The bottom edge remains resizable vertically. The 4.0 UI draws rounded
window and dialog surfaces with Slint's native DPI-aware geometry and keeps
nine-slice assets for the subtle shadows and dimmed backdrop. This avoids
relying on unsupported software-renderer shadow and rounded clipping
features. The Settings footer links directly to the fork maintainer's GitHub.

To run next to an installed PinIt without touching it, set `PINIT_DEV=1`.
The build then uses `%LOCALAPPDATA%\PinIt-dev`, its own single-instance lock,
and its own Run-key value. The default shortcuts will be reported as
unavailable while the installed copy holds them; pick spare ones in
**Shortcuts** (for example `Ctrl+Alt+Shift+F13`).

## Layout

| File | What it does |
|---|---|
| `ui/theme.slint` | Colours, type and motion for light and dark mode |
| `ui/controls.slint` | Branded button, check box, slider, key caps, sheet |
| `ui/app.slint` | The window, window picker, shortcut editor and settings |
| `src/main.rs` | Wires the UI, the pins, the tray and the hotkeys together |
| `src/pins.rs` | Which windows are pinned; re-applying topmost; restoring saved pins |
| `src/win.rs` | Every Win32 call on other apps' windows |
| `src/host.rs` | Hidden helper window: tray icon and menu, hotkeys, shutdown, second launch |
| `src/flash.rs` | The outline drawn around a window when it is pinned |
| `src/store.rs` | `pinned.json` |
| `src/shortcuts.rs` | Shortcut strings such as `super+ctrl+KeyT` |
| `src/autostart.rs` | The HKCU Run key |
| `src/logfile.rs` | `pinit.log`: process names only, never window titles |

## Not done yet

- Mica backdrop on Windows 11
- The maximize button: the window has a fixed width, but Windows still shows it
