# PinIt 5.0.0 — Mini window and smarter topmost management

PinIt 5.0.0 adds a compact always-on-top control, independent management of pinned windows, and more reliable control of PinIt's own window on Windows 10 and 11.

## What's new since 4.0.0

- **Rounded mini window:** Click the PinIt logo/name at the left of the title bar to collapse to a small, always-on-top window. Drag the mini window to reposition it, and click to expand back to the previous size. Your original manual self-pin preference is preserved.
- **Independent self-pinning:** The title-bar pin now controls PinIt's own long-term topmost state, with distinct on/off icons.
- **Focus-based self pinning:** By default, PinIt temporarily becomes topmost while it has keyboard focus and returns to the previous long-term state on focus loss. Disable this in Settings if preferred.
- **Absolute topmost:** Enabled by default, this re-raises PinIt on foreground-window changes when its title-bar pin is on or it is in mini mode, without taking focus from another app.
- **Optional aggressive mode:** Disabled by default. A 500-ms check only re-raises PinIt if an overlapping topmost window is actually above it. It is inactive when absolute topmost is off or PinIt is minimized/hidden.
- **More reliable wake-up:** Tray click and show/hide hotkey now use the same rule: a minimized, hidden, or background PinIt is brought forward instead of being unexpectedly hidden. Explicit wake-up also raises a manually pinned or compact PinIt even when focus-based pinning is off.
- **Manage without pinning:** Each managed window has its own pin/unpin toggle. Turning it off keeps that window in the list, with its opacity and click-through preferences. The × button unpins, restores the original window state, and removes the entry; **Unpin all** keeps entries while turning their pins off.
- **Choose or drag to pin:** The main action is split into the existing window picker and a drag-to-target action.
- **Optional full exit on close:** Closing to the tray remains the default. The new **Exit PinIt instead of hiding to tray** setting makes Close fully exit the process instead. Like tray **Quit PinIt**, a full exit restores managed windows and clears the remembered management list.
- **Smaller compact hot zone:** Only the left logo/name toggles compact mode; the empty middle part of the title bar remains available for dragging.

## Downloads

- `PinIt_5.0.0_x64-setup.exe`: Windows installer (per user).
- `PinIt-setup-x64.exe`: equivalent installer, stable filename for latest-release links.
- `PinIt-portable-x64.zip`: portable single-executable build.
- `SHA256SUMS.txt`: SHA-256 checksums for verifying downloads.

Existing `%LOCALAPPDATA%\PinIt\pinned.json` state and the installer AppId remain compatible. No configuration reset is required; new settings use the defaults described above.

Windows CI runs `cargo test --locked` and `cargo build --release --locked`, packages the installer and portable ZIP, and verifies checksums before publication. Physical Windows desktop testing, exclusive full-screen rendering, and interactions with third-party topmost tools remain environment-specific; "absolute" topmost is best-effort within the Windows Z-order.

**Note:** The binaries are not Authenticode-signed, so Windows SmartScreen may present an unrecognized-app warning.

PinIt is an Apache-2.0 fork of the original project by Saqlain Abbas (Razee4315). The interface uses Slint with its Royalty-free Desktop license.
