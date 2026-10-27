<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/assets/wordmark-dark.png">
    <img src="docs/assets/wordmark-light.png" width="360" alt="PinIt — pin any window always on top">
  </picture>
</p>

<p align="center">
  <b>Keep any window always on top on Windows 11 & 10 — instantly, with a global hotkey.</b>
</p>

<p align="center">
  <a href="https://github.com/snownico0722/Pin-It/releases/latest"><img src="https://img.shields.io/github/v/release/snownico0722/Pin-It?style=flat-square" alt="Latest release"></a>
  <a href="https://github.com/snownico0722/Pin-It/releases"><img src="https://img.shields.io/github/downloads/snownico0722/Pin-It/total?style=flat-square" alt="Downloads"></a>
  <a href="https://github.com/snownico0722/Pin-It/releases/latest"><img src="https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-0078D6?style=flat-square" alt="Platform"></a>
  <a href="https://slint.dev"><img src="https://img.shields.io/badge/built%20with-Rust%20%26%20Slint-427BB9?style=flat-square" alt="Built with Rust and Slint"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-green?style=flat-square" alt="License: Apache 2.0"></a>
</p>

Press `Win+Ctrl+T` and the focused window stays on top of everything else. Slide its opacity down to see through it. Restart your PC and PinIt re-pins it automatically. A single-purpose alternative to installing a whole utility suite — implemented as a single native Rust/Slint executable using the Windows API.

<p align="center">
  <img src="docs/assets/pin-it-demo.gif" width="720" alt="PinIt in action: pinning a Notepad window always on top with Win+Ctrl+T, then fading its opacity to see through it">
  <br>
  <em>Pin a window, fade it, and keep working — all from one hotkey</em>
</p>

## Download

<a href="https://slint.dev"><img src="https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png" alt="Made with Slint" height="26"></a>


**[⬇ Download the latest release](https://github.com/snownico0722/Pin-It/releases/latest)** — Windows 10 & 11, free.

| File | What it is |
|------|------------|
| `PinIt_x.y.z_x64-setup.exe` | Installer (recommended) — Start Menu shortcut + uninstaller |
| `PinIt-portable-x64.zip` | Portable — unzip and run, no installation |

> **Note:** The installers are not yet code-signed, so Windows SmartScreen may show "Windows protected your PC". Click **More info → Run anyway**. PinIt is fully open source (Apache 2.0) — audit the code or build it yourself from this repository.

## Features

- **Global hotkey pinning** — `Win+Ctrl+T` pins/unpins the focused window, with a brief outline around it so you see it happen. No clicking through menus.
- **Per-window transparency** — make any pinned window see-through with `Win+Ctrl+=` / `Win+Ctrl+-` or a slider. Great for reference docs, video calls, or notes over your work.
- **Pins survive restarts** — PinIt remembers what you pinned (and its opacity). After a restart it pins each window again as soon as that window is open.
- **Windows 11 topmost re-enforcement** — Win11's compositor sometimes strips the always-on-top flag; PinIt re-applies it automatically.
- **Click-through** — let mouse clicks pass through a faded window to whatever is behind it.
- **System tray app** — closes to the tray and stays out of your way; the tray menu lists your pinned windows. Optional start-with-Windows.
- **Light and dark** — follows your Windows colour mode.
- **Tiny and fast** — native Rust/Slint talking directly to the Windows API. Minimal RAM, instant response.

## Keyboard Shortcuts

| Action | Default shortcut |
|--------|------------------|
| Pin / unpin focused window | `Win` + `Ctrl` + `T` |
| Increase opacity | `Win` + `Ctrl` + `=` |
| Decrease opacity | `Win` + `Ctrl` + `-` |
| Show / hide PinIt | `Win` + `Ctrl` + `P` |

## How PinIt compares

PowerToys is great when you want twenty utilities. PinIt is for when you want exactly one, done properly:

| Feature | **PinIt** | PowerToys <sub>Always On Top module</sub> | DeskPins <sub>classic freeware</sub> |
|---|:---:|:---:|:---:|
| Price | ✅ Free | ✅ Free | ✅ Free |
| Single-purpose, lightweight | ✅ **Yes** | ❌ Full utility suite | ✅ Yes |
| Global hotkey to pin | ✅ `Win+Ctrl+T` | ✅ Yes | ✅ Yes |
| True per-window transparency | ✅ **20 to 100%** | ❌ No ¹ | ❌ No |
| Pins persist across restarts | ✅ **Automatic** | ❌ Forgets on reboot | ❌ No |
| Actively maintained | ✅ Yes | ✅ Yes | ❌ Last release 2018 |

> **The short version:** if pinning is all you need, PinIt is the smallest tool that does all of it, and the only one that remembers your pins after a reboot.

¹ PowerToys' "Opacity" setting changes the highlight *border* around pinned windows, not the window content itself. True window transparency is a long-standing open feature request ([#26049](https://github.com/microsoft/PowerToys/issues/26049)).

## FAQ

### How do I keep a window always on top in Windows 11?

Windows has no built-in always-on-top button. Install PinIt, click the window you want to keep visible, and press `Win+Ctrl+T` — it stays on top of every other window until you unpin it.

### What is the keyboard shortcut to pin a window on top?

PinIt's default is `Win+Ctrl+T` to toggle pinning.

### Can I make a window transparent / see-through on Windows?

Yes — pin a window with PinIt, then press `Win+Ctrl+-` to fade it (down to 20% opacity) or `Win+Ctrl+=` to make it solid again. Each pinned window keeps its own opacity level.

### Do my pinned windows stay on top after I restart?

Yes. PinIt saves your pins (app, window title and opacity) to `%LOCALAPPDATA%\PinIt`. After a restart, each saved window is pinned again as soon as it is open with the same title — windows that aren't open yet are listed as "Waiting" in PinIt until they are. Quitting PinIt yourself (tray → Quit) unpins everything and clears the list. Neither PowerToys nor DeskPins remembers pins at all.

### Does it work with apps running as administrator?

Windows security (UIPI) prevents normal apps from modifying elevated windows. To pin a window that's running as administrator, run PinIt as administrator too.

### Is PinIt free and open source?

Yes — PinIt is completely free and open source under the [Apache 2.0 license](LICENSE). Use it, modify it, and redistribute it, including commercially.

## Building from source

### Prerequisites

- [Rust 1.88 or newer](https://rustup.rs/) with the MSVC Windows toolchain
- Windows SDK / Visual Studio Build Tools (for executable resource compilation)

### Build

```bash
# Clone the repository
git clone https://github.com/snownico0722/Pin-It.git
cd Pin-It/rust

cargo test --locked
cargo build --release --locked
```

The portable executable is `rust/target/release/pinit.exe`. CI packages it as `PinIt.exe` and builds the [Inno Setup](https://jrsoftware.org/isinfo.php) installer.

> Windows CI (`.github/workflows/build.yml`) tests and packages every change. The release workflow (`.github/workflows/release.yml`) publishes CI-verified installer and portable ZIP assets with SHA-256 checksums.

## Why PinIt?

As a developer moving between Linux and Windows, I always missed the native ability to keep any window on top. While Linux desktop environments often have this built-in, Windows options were limited.

The most common solution, Microsoft PowerToys, comes bundled with dozens of other utilities I didn't need. I wanted a lightweight, singular-purpose tool that does one thing and does it well — without the bloat. PinIt was born from the desire for a clean, simple, and resource-efficient alternative that feels like a native part of the system.

## Tech Stack

- **Language**: Rust 2024
- **UI**: Slint 1.18 (software renderer) + Windows API
- **Build**: Cargo + Inno Setup

> Originally built with Rust + Tauri (v1.x). The previous implementation is preserved on the [`legacy-tauri`](https://github.com/Razee4315/Pin-It/tree/legacy-tauri) branch.

## License

PinIt is open source under the **[Apache License 2.0](LICENSE)** — free to use, modify, and redistribute, including for commercial purposes.

## Author

**Saqlain Abbas**
Email: saqlainrazee@gmail.com

GitHub: [@Razee4315](https://github.com/Razee4315)

## Languages / 语言

PinIt 4 bundles English and Simplified Chinese inside the single executable. The default follows Windows language. Choose **Settings → Language → Follow system / English / 简体中文** to switch at runtime; the choice is saved locally.

PinIt 4 单文件内置简体中文与英文。默认跟随 Windows 语言，可在「设置 → 语言」中随时切换并保存偏好。

## Version 4.0.0 (fork)

PinIt 4.0.0 includes the compact frameless UI, drag-to-pin picker, native-resolution window icons, built-in Chinese/English translations, and automatic pin restoration. This release refines native-DPI rounded corners, softens shadows, expands the draggable titlebar area, and fixes opacity restoration and duplicate saved-pin handling. See [Releases](https://github.com/snownico0722/Pin-It/releases) for CI-built installer and portable assets (Windows binaries are not Authenticode-signed).

Fork maintained at [@snownico0722](https://github.com/snownico0722); original project by [Saqlain Abbas / Razee4315](https://github.com/Razee4315/Pin-It).
