# PinIt 3.1.0 — Simplified Chinese and reliability fixes

## Changes

- Bundled Simplified Chinese and English translations in the single-file Rust/Slint executable.
- Language choice: system default, English or 简体中文; saved between runs.
- Localize tray menus, keyboard shortcuts, error feedback and status messages.
- Preserve and restore original layered-window alpha/color key when adjusting transparency.
- Restore click-through state in both directions when unpinning.
- Re-pin matched saved windows when they open in the background, not only when focused.
- Prevent corruption of per-pixel-alpha windows whose original layered attributes cannot be read.
- Permit swapping registered global hotkey combinations without spurious conflicts.

## Install

Use the x64 installer or extract the portable ZIP. The installer is not signed with an Authenticode certificate. This is a fork of the original PinIt project and remains under Apache 2.0.

Release assets must be built from this commit and pass the Rust test suite and SHA-256 validation.
