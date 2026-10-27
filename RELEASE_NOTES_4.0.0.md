# PinIt 4.0.0 — Reliable window controls and refined desktop UI

PinIt 4.0.0 brings together the Rust/Slint window controls and the latest usability and reliability fixes. This is an update to the 3.3.0 release with no configuration reset and no new pinning restrictions.

## What's improved since 3.3.0

- **Smoother rounded windows:** main window and settings/shortcut panels use DPI-aware native rounded corners; bitmap assets provide soft shadows only, reducing visible jagged edges.
- **Subtler shadows:** removes the previous heavy stacked halo in light and dark themes.
- **Easier window dragging:** the whole noninteractive header area moves the window while controls, resizing, and drag-to-pin remain unchanged.
- **Safer opacity changes:** preserves colour-key transparency and avoids leaving a normal window layered if an opacity change fails.
- **More accurate unpin feedback:** attempts all relevant restoration operations and reports incomplete restoration instead of silently claiming full success. Cancelling a pin is still allowed.
- **More reliable saved pins:** pinning one window no longer consumes every waiting entry with the same process and title; failures to restore opacity or click-through are recorded in the local log.

## Existing features

- Pin/unpin using `Win+Ctrl+T` or drag the pushpin onto a window.
- Window opacity (20–100%) and optional mouse click-through.
- Auto re-pin windows remembered from previous sessions, global hotkeys, tray controls, and Chinese/English UI.
- Native single-executable Rust/Slint build for Windows 10 and 11.

## Install

- `PinIt_4.0.0_x64-setup.exe`: per-user Windows installer.
- `PinIt-portable-x64.zip`: portable executable, no installer needed.
- `SHA256SUMS.txt`: checksums for verifying release downloads.

Existing `%LOCALAPPDATA%\PinIt\pinned.json` settings and installer AppId remain compatible. No new permission requests, locks, or extra confirmation steps were added.

Build verification: Windows CI runs `cargo test --locked`, `cargo build --release --locked`, produces the installer and portable ZIP, and checks generated SHA-256 hashes before publishing. Real Windows compatibility testing across third-party apps and display scaling is separate from CI.

**Note:** These Windows binaries are not Authenticode-signed, so Microsoft SmartScreen may show a warning.

PinIt is an Apache-2.0 fork of the original project by Saqlain Abbas (Razee4315). The interface uses Slint with its Royalty-free Desktop license.
