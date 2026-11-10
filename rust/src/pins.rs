//! pins — managed windows, their always-on-top state, and keeping it applied.
//!
//! Pure bookkeeping over `win`: no UI, no timers. The app calls `reenforce`
//! on a timer and when the foreground window changes, and saves whatever
//! `saved()` returns.

use crate::store::{self, SavedPin};
use crate::win::{self, Hwnd};

#[derive(Clone, Debug)]
pub struct Pinned {
    pub hwnd: Hwnd,
    pub title: String,
    pub process: String,
    pub enabled: bool,
    pub opacity: i32, // percent
    pub click_through: bool,
    was_layered: bool,       // had WS_EX_LAYERED before PinIt touched it
    original_layered: Option<win::LayeredAttributes>,
    was_topmost: bool,       // was already always-on-top (Task Manager, players)
    was_click_through: bool, // already let clicks through
    opacity_changed: bool,   // PinIt changed its opacity, so undo that on removal
}

#[derive(Debug, PartialEq)]
pub enum PinError {
    NoWindow,
    Gone,
    NotPinnable,
    Blocked(String),
    NotPinned,
    UnsupportedTransparency,
    UnpinFailed,
    RestoreFailed,
}

impl PinError {
    pub fn message(&self, chinese: bool) -> String {
        if chinese {
            return match self {
                PinError::NoWindow => "没有可置顶的窗口，请先选择窗口。".into(),
                PinError::Gone => "目标窗口已不存在。".into(),
                PinError::NotPinnable => "该窗口无法置顶。".into(),
                PinError::Blocked(app) => {
                    let app = if app.is_empty() { "该窗口" } else { app };
                    format!("无法置顶 {app}，它可能以管理员身份运行。")
                }
                PinError::NotPinned => "请先将窗口加入管理。".into(),
                PinError::UnsupportedTransparency => "目标窗口使用特殊透明模式，无法安全恢复原始状态。".into(),
                PinError::UnpinFailed => "无法取消窗口置顶，请检查目标窗口。".into(),
                PinError::RestoreFailed => "已从管理列表移除，但未能完全恢复目标窗口状态。".into(),
            };
        }
        match self {
            PinError::NoWindow => "No window to pin. Click a window first.".into(),
            PinError::Gone => "That window no longer exists.".into(),
            PinError::NotPinnable => "That window can't be pinned.".into(),
            PinError::Blocked(app) => {
                let app = if app.is_empty() { "this window" } else { app };
                format!("Can't pin {app}. It may be running as administrator.")
            }
            PinError::NotPinned => "Add this window to PinIt first.".into(),
            PinError::UnsupportedTransparency => "This window uses custom layered transparency that cannot be safely restored.".into(),
            PinError::UnpinFailed => "Could not turn off always-on-top for this window.".into(),
            PinError::RestoreFailed => "Removed from PinIt, but could not fully restore the window's original state.".into(),
        }
    }
}

#[derive(Default)]
pub struct Reenforced {
    pub list_changed: bool,
    pub titles_changed: bool,
    pub restored: Vec<Hwnd>,
}

#[derive(Default)]
pub struct Pins {
    pinned: Vec<Pinned>,
    pending: Vec<SavedPin>,
    session_ending: bool,
}

impl Pins {
    /// All managed live windows, including entries with always-on-top turned off.
    pub fn pinned(&self) -> &[Pinned] {
        &self.pinned
    }

    pub fn pending(&self) -> &[SavedPin] {
        &self.pending
    }

    pub fn is_pinned(&self, hwnd: Hwnd) -> bool {
        self.pinned.iter().any(|p| p.hwnd == hwnd && p.enabled)
    }

    pub fn is_managed(&self, hwnd: Hwnd) -> bool {
        self.pinned.iter().any(|p| p.hwnd == hwnd)
    }

    pub fn is_active(&self) -> bool {
        !self.pinned.is_empty() || !self.pending.is_empty()
    }

    fn find(&mut self, hwnd: Hwnd) -> Option<&mut Pinned> {
        self.pinned.iter_mut().find(|p| p.hwnd == hwnd)
    }

    pub fn pin(&mut self, hwnd: Hwnd) -> Result<(), PinError> {
        if self.is_pinned(hwnd) {
            return Ok(());
        }
        if self.is_managed(hwnd) {
            return self.set_enabled(hwnd, true);
        }
        self.manage(hwnd, true)
    }

    fn manage(&mut self, hwnd: Hwnd, enabled: bool) -> Result<(), PinError> {
        if !win::is_valid_window(hwnd) {
            return Err(PinError::Gone);
        }
        if !win::is_pinnable(hwnd) {
            return Err(PinError::NotPinnable); // desktop, taskbar, Start, PinIt
        }
        let title = win::window_title(hwnd);
        let process = win::process_name(hwnd);
        let was_topmost = win::is_topmost(hwnd);
        let was_layered = win::is_layered(hwnd);
        let original_layered = win::layered_attributes(hwnd);
        Self::apply_enabled(hwnd, enabled, &process)?;
        crate::logfile::info!("Managing a window of {process} (pinned: {enabled})");
        let was_click_through = win::is_click_through(hwnd);
        store::consume_one_matching(&mut self.pending, &process, &title);
        self.pinned.push(Pinned {
            hwnd,
            title,
            process,
            enabled,
            opacity: 100,
            click_through: was_click_through,
            was_layered,
            original_layered,
            was_topmost,
            was_click_through,
            opacity_changed: false,
        });
        Ok(())
    }

    fn apply_enabled(hwnd: Hwnd, enabled: bool, process: &str) -> Result<(), PinError> {
        if !win::is_valid_window(hwnd) {
            return Err(PinError::Gone);
        }
        // UIPI can silently ignore SetWindowPos; confirm the requested style.
        let applied = if enabled { win::apply_topmost(hwnd) } else { win::remove_topmost(hwnd) };
        if !applied || win::is_topmost(hwnd) != enabled {
            crate::logfile::warning!("Could not set always-on-top to {enabled} for {process}");
            return Err(if enabled { PinError::Blocked(process.into()) } else { PinError::UnpinFailed });
        }
        Ok(())
    }

    /// Toggle only always-on-top. Opacity and click-through stay under management.
    pub fn set_enabled(&mut self, hwnd: Hwnd, enabled: bool) -> Result<(), PinError> {
        let p = self.find(hwnd).ok_or(PinError::NotPinned)?;
        Self::apply_enabled(hwnd, enabled, &p.process)?;
        p.enabled = enabled;
        Ok(())
    }

    /// Attempt every restoration even when an earlier Win32 operation failed.
    /// The UI still allows removing the pin; it reports partial failure instead
    /// of incorrectly claiming that the foreign window was fully restored.
    fn release(p: &Pinned, force_unpin: bool) -> bool {
        if !win::is_valid_window(p.hwnd) {
            return true; // closed windows need no restoration
        }
        let mut ok = true;
        if win::is_click_through(p.hwnd) != p.was_click_through {
            ok &= win::set_click_through(p.hwnd, p.was_click_through);
        }
        if p.opacity_changed {
            ok &= win::restore_opacity(p.hwnd, p.was_layered, p.original_layered.as_ref());
        }
        if force_unpin || !p.was_topmost {
            ok &= win::remove_topmost(p.hwnd) && !win::is_topmost(p.hwnd);
        }
        if !ok {
            crate::logfile::warning!("Could not completely restore a window of {}", p.process);
        }
        ok
    }

    /// Remove the managed entry, turn off always-on-top, and restore other styles.
    pub fn unpin(&mut self, hwnd: Hwnd) -> Option<(Pinned, bool)> {
        let i = self.pinned.iter().position(|p| p.hwnd == hwnd)?;
        let p = self.pinned.remove(i);
        let restored = Self::release(&p, true);
        Some((p, restored))
    }

    /// Turn off always-on-top for every entry, keeping the management list.
    /// Waiting entries also stay in the list and will be restored unpinned.
    /// Returns how many entries changed and how many native changes failed.
    pub fn unpin_all(&mut self) -> (usize, usize) {
        let mut n = 0;
        let mut failures = 0;
        for p in self.pinned.iter_mut().filter(|p| p.enabled) {
            if Self::apply_enabled(p.hwnd, false, &p.process).is_ok() {
                p.enabled = false;
                n += 1;
            } else {
                failures += 1;
            }
        }
        for p in self.pending.iter_mut().filter(|p| p.enabled) {
            p.enabled = false;
            n += 1;
        }
        (n, failures)
    }

    /// Ok(true) = now pinned, Ok(false) = now unpinned.
    pub fn toggle(&mut self, hwnd: Hwnd) -> Result<bool, PinError> {
        if hwnd == 0 {
            return Err(PinError::NoWindow);
        }
        if self.is_managed(hwnd) {
            let enabled = !self.is_pinned(hwnd);
            return self.set_enabled(hwnd, enabled).map(|()| enabled);
        }
        self.pin(hwnd).map(|_| true)
    }

    pub fn set_opacity(&mut self, hwnd: Hwnd, percent: i32) -> Result<i32, PinError> {
        let p = self.find(hwnd).ok_or(PinError::NotPinned)?;
        // Avoid damaging per-pixel alpha windows whose existing layered
        // attributes cannot be read and restored.
        if p.was_layered && p.original_layered.is_none() {
            return Err(PinError::UnsupportedTransparency);
        }
        let percent = percent.clamp(win::MIN_OPACITY, win::MAX_OPACITY);
        if !win::set_opacity_percent(hwnd, percent) {
            return Err(PinError::Gone);
        }
        p.opacity = percent;
        p.opacity_changed = true;
        Ok(percent)
    }

    pub fn adjust_opacity(&mut self, hwnd: Hwnd, delta: i32) -> Result<i32, PinError> {
        let current = self.find(hwnd).ok_or(PinError::NotPinned)?.opacity;
        self.set_opacity(hwnd, current + delta)
    }

    pub fn set_click_through(&mut self, hwnd: Hwnd, enabled: bool) -> Result<(), PinError> {
        let opacity = self.find(hwnd).ok_or(PinError::NotPinned)?.opacity;
        // Windows only honours click-through on a layered window, and a layered
        // window needs an alpha to be drawn at all: setting the opacity does both.
        if enabled && !win::is_layered(hwnd) {
            self.set_opacity(hwnd, opacity)?;
        }
        if !win::set_click_through(hwnd, enabled) {
            return Err(PinError::Gone);
        }
        if let Some(p) = self.find(hwnd) {
            p.click_through = enabled;
        }
        Ok(())
    }

    /// Windows 11 sometimes drops the topmost flag; put it back, sweep out
    /// closed windows, follow title changes, and pin a pending window that has
    /// just opened.
    pub fn reenforce(&mut self) -> Reenforced {
        let mut out = Reenforced::default();
        // While Windows is closing every app, a window disappearing is not the
        // user closing it; dropping its pin would lose it across the restart.
        if self.session_ending {
            return out;
        }
        let before = self.pinned.len();
        self.pinned.retain(|p| win::is_valid_window(p.hwnd));
        out.list_changed = self.pinned.len() != before;

        for p in &mut self.pinned {
            if p.enabled && !win::is_topmost(p.hwnd) {
                win::apply_topmost(p.hwnd);
            }
            let title = win::window_title(p.hwnd);
            if !title.is_empty() && title != p.title {
                p.title = title;
                out.titles_changed = true;
            }
        }

        // Apps can open in the background after login. Scan all eligible
        // windows only while saved pins are waiting, on the existing timer.
        if !self.pending.is_empty() {
            for w in win::enumerate_windows() {
                if self.is_managed(w.hwnd) { continue; }
                if let Some(saved) = self.pending.iter().find(|s| s.matches(&w.process, &w.title)).cloned() {
                    if self.apply_saved(w.hwnd, &saved) {
                        if saved.enabled {
                            out.restored.push(w.hwnd);
                        }
                        out.list_changed = true;
                    }
                }
            }
        }
        out
    }

    fn apply_saved(&mut self, hwnd: Hwnd, saved: &SavedPin) -> bool {
        if self.manage(hwnd, saved.enabled).is_err() {
            return false;
        }
        // After a crash the window still carries PinIt's changes from last
        // time, which manage() just recorded as the app's own. The saved flags
        // know better.
        if let Some(p) = self.find(hwnd) {
            p.was_layered &= saved.was_layered;
            p.original_layered = saved.original_layered.clone().or(p.original_layered.clone());
            p.was_topmost &= saved.was_topmost;
            p.was_click_through &= saved.was_click_through;
        }
        let percent = win::alpha_to_percent(saved.opacity);
        if percent < 100 && self.set_opacity(hwnd, percent).is_err() {
            crate::logfile::warning!("Could not restore opacity for {}", saved.process_name);
        }
        if saved.click_through && self.set_click_through(hwnd, true).is_err() {
            crate::logfile::warning!("Could not restore click-through for {}", saved.process_name);
        }
        true
    }

    /// At startup: restore saved windows that are open; the rest wait for
    /// their window (PinIt usually starts before the apps after a reboot).
    pub fn restore(&mut self, saved: Vec<SavedPin>) -> Vec<Hwnd> {
        self.pending = saved.clone();
        let live = win::enumerate_windows();
        let mut restored = Vec::new();
        for s in &saved {
            if let Some(w) = live.iter().find(|w| !self.is_managed(w.hwnd) && s.matches(&w.process, &w.title)) {
                if self.apply_saved(w.hwnd, s) {
                    if s.enabled {
                        restored.push(w.hwnd);
                    }
                }
            }
        }
        restored
    }

    pub fn forget_pending(&mut self, index: usize) {
        if index < self.pending.len() {
            self.pending.remove(index);
        }
    }

    pub fn toggle_pending(&mut self, index: usize) -> Option<bool> {
        let pin = self.pending.get_mut(index)?;
        pin.enabled = !pin.enabled;
        Some(pin.enabled)
    }

    pub fn set_session_ending(&mut self, ending: bool) {
        self.session_ending = ending;
    }

    /// On quit: give every window back. After a manual quit the pins are
    /// forgotten; at logoff/shutdown they are kept for the next login.
    pub fn release_all(&mut self) {
        for p in &self.pinned {
            Self::release(p, false);
        }
        if !self.session_ending {
            self.pinned.clear();
            self.pending.clear();
        }
    }

    /// What goes into pinned.json: live pins, then the ones still waiting.
    pub fn saved(&self) -> Vec<SavedPin> {
        self.pinned
            .iter()
            .map(|p| SavedPin {
                process_name: p.process.clone(),
                title: p.title.clone(),
                enabled: p.enabled,
                opacity: win::percent_to_alpha(p.opacity) as i32,
                click_through: p.click_through,
                was_layered: p.was_layered,
                original_layered: p.original_layered.clone(),
                was_topmost: p.was_topmost,
                was_click_through: p.was_click_through,
                extra: Default::default(),
            })
            .chain(self.pending.iter().cloned())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpin_all_retains_waiting_entries_and_their_settings() {
        let pending: SavedPin = serde_json::from_str(
            r#"{"process_name":"Code.exe","title":"Notes","opacity":128,"click_through":true}"#,
        ).unwrap();
        let mut pins = Pins { pending: vec![pending.clone()], ..Default::default() };

        assert_eq!(pins.unpin_all(), (1, 0));
        assert_eq!(pins.pending().len(), 1);
        assert!(!pins.pending()[0].enabled);
        assert_eq!(pins.pending()[0].opacity, pending.opacity);
        assert_eq!(pins.pending()[0].click_through, pending.click_through);
        assert!(pins.is_active(), "waiting entries still need the discovery timer");
        assert_eq!(pins.saved(), pins.pending());
        assert_eq!(pins.unpin_all(), (0, 0));
        assert_eq!(pins.toggle_pending(0), Some(true));
        pins.forget_pending(0);
        assert!(!pins.is_active());
    }
}
