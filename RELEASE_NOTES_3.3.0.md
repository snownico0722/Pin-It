# PinIt 3.3.0 — Drag-to-pin, refined window chrome and reliability

## What's new

- **Frameless window:** a compact draggable title area with dedicated minimize and close-to-tray controls.
- **Drag-to-pin:** drag the pushpin from the header onto another application's window to add it to the pinned list. The pointer changes to a crosshair while dragging. Dragging an already pinned window never unpins it.
- **Sharp icons:** prefer native high-resolution ICO entries when listing apps; the PinIt header uses a vector mark instead of a blurry downsized bitmap.
- **Rounded UI that actually renders:** the Slint software renderer lacks support for drop shadows and rounded child clipping. Version 3.3 uses antialiased, alpha-masked nine-slice assets for the outer frame, sheet shadows, and dimmed modal backdrop in both light and dark themes, avoiding square overlay artifacts.
- **Less UI clutter:** the modifier requirement appears next to the Shortcuts sheet heading. Settings ends with a GitHub icon and `PinIt 3.3.0`; clicking opens the fork maintainer's GitHub profile.

## Included from 3.1

- Simplified Chinese / English interface and runtime language selection.
- Restore layered-window opacity and original click-through settings.
- Automatically re-pin saved windows and validate hotkey conflicts.

## Install

Download `PinIt_3.3.0_x64-setup.exe` or `PinIt-portable-x64.zip` below. Existing per-user settings and pins remain compatible; the installer retains the same AppId. The binaries are not Authenticode-signed.

The Windows release pipeline runs `cargo test --locked`, `cargo build --release --locked`, constructs the installer and portable ZIP, and checks every file against `SHA256SUMS.txt` before publishing. Runtime visual verification at 100%, 150%, and 200% Windows display scaling remains recommended.

This fork is based on the original PinIt by Saqlain Abbas (Razee4315), under Apache-2.0. The UI uses Slint with the Royalty-free Desktop license and attribution on the repository download page.
