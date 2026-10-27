//! win — the Win32 calls PinIt makes on other apps' windows.
//!
//! Everything that touches a foreign HWND lives here, so the rest of the app
//! deals in plain `Hwnd` values and never needs `unsafe`.

use std::mem::size_of;

use windows::Win32::Foundation::{CloseHandle, COLORREF, HWND, LPARAM, POINT, RECT, TRUE, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_CLOAKED,
    DWMWA_EXTENDED_FRAME_BOUNDS, DWMWA_USE_IMMERSIVE_DARK_MODE,
};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetObjectW, SelectObject,
    BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
};
use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, GetCapture, ReleaseCapture, SetCapture, VK_ESCAPE, VK_LBUTTON};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, PCWSTR, PWSTR};

/// A window handle as a plain number, so it can be stored, compared, hashed
/// and sent across threads.
pub type Hwnd = isize;

pub const MIN_OPACITY: i32 = 20;
pub const MAX_OPACITY: i32 = 100;

/// Original layered attributes, captured before PinIt modifies another window.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LayeredAttributes {
    pub color_key: u32,
    pub alpha: u8,
    pub flags: u32,
}

pub fn layered_attributes(hwnd: Hwnd) -> Option<LayeredAttributes> {
    if !is_layered(hwnd) { return None; }
    let mut color = COLORREF(0);
    let mut alpha: u8 = 255;
    let mut flags = LAYERED_WINDOW_ATTRIBUTES_FLAGS(0);
    unsafe {
        GetLayeredWindowAttributes(h(hwnd), Some(&mut color), Some(&mut alpha), Some(&mut flags)).ok()?;
    }
    Some(LayeredAttributes { color_key: color.0, alpha, flags: flags.0 })
}

fn h(hwnd: Hwnd) -> HWND {
    HWND(hwnd as *mut _)
}

pub fn percent_to_alpha(percent: i32) -> u8 {
    let p = percent.clamp(0, 100);
    ((p * 255 + 50) / 100) as u8
}

pub fn alpha_to_percent(alpha: i32) -> i32 {
    let a = alpha.clamp(0, 255);
    (a * 100 + 127) / 255
}

pub fn window_title(hwnd: Hwnd) -> String {
    unsafe {
        let len = GetWindowTextLengthW(h(hwnd));
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        let copied = GetWindowTextW(h(hwnd), &mut buf);
        String::from_utf16_lossy(&buf[..copied.max(0) as usize])
    }
}

/// "notepad.exe" — the file name of the process that owns the window.
pub fn process_name(hwnd: Hwnd) -> String {
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(h(hwnd), Some(&mut pid));
        if pid == 0 {
            return String::new();
        }
        let Ok(proc) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return String::new();
        };
        let mut buf = [0u16; 1024];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(proc, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut size);
        let _ = CloseHandle(proc);
        if ok.is_err() {
            return String::new();
        }
        let full = String::from_utf16_lossy(&buf[..size as usize]);
        match full.rfind('\\') {
            Some(i) => full[i + 1..].to_string(),
            None => full,
        }
    }
}

pub fn foreground_window() -> Hwnd {
    unsafe { GetForegroundWindow().0 as Hwnd }
}

pub fn is_valid_window(hwnd: Hwnd) -> bool {
    hwnd != 0 && unsafe { IsWindow(Some(h(hwnd))).as_bool() }
}

fn ex_style(hwnd: Hwnd) -> u32 {
    unsafe { GetWindowLongW(h(hwnd), GWL_EXSTYLE) as u32 }
}

fn set_ex_style(hwnd: Hwnd, style: u32) -> bool {
    unsafe { SetWindowLongW(h(hwnd), GWL_EXSTYLE, style as i32); }
    ex_style(hwnd) == style
}

pub fn is_topmost(hwnd: Hwnd) -> bool {
    ex_style(hwnd) & WS_EX_TOPMOST.0 != 0
}

pub fn is_layered(hwnd: Hwnd) -> bool {
    ex_style(hwnd) & WS_EX_LAYERED.0 != 0
}

pub fn is_click_through(hwnd: Hwnd) -> bool {
    ex_style(hwnd) & WS_EX_TRANSPARENT.0 != 0
}

pub fn class_name(hwnd: Hwnd) -> String {
    let mut buf = [0u16; 256];
    let len = unsafe { GetClassNameW(h(hwnd), &mut buf) };
    String::from_utf16_lossy(&buf[..len.max(0) as usize])
}

/// The window's visible frame in physical pixels (without the invisible
/// resize borders Windows 10/11 add around it).
pub fn frame_rect(hwnd: Hwnd) -> Option<RECT> {
    unsafe {
        let mut r = RECT::default();
        let dwm = DwmGetWindowAttribute(
            h(hwnd),
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut r as *mut _ as *mut _,
            size_of::<RECT>() as u32,
        );
        if dwm.is_err() && GetWindowRect(h(hwnd), &mut r).is_err() {
            return None;
        }
        (r.right > r.left && r.bottom > r.top).then_some(r)
    }
}

/// Parts of the Windows shell that must never be pinned.
pub fn is_shell_class(class: &str) -> bool {
    const SHELL_CLASSES: &[&str] = &[
        "Progman",                             // desktop
        "WorkerW",                             // desktop (wallpaper host)
        "Shell_TrayWnd",                       // taskbar
        "Shell_SecondaryTrayWnd",              // taskbar on other monitors
        "NotifyIconOverflowWindow",            // tray overflow (Windows 10)
        "TopLevelWindowForOverflowXamlIsland", // tray overflow (Windows 11)
        "Windows.UI.Core.CoreWindow",          // Start, Search, Action Center
        "XamlExplorerHostIslandWindow",        // Task View / Alt+Tab (Windows 11)
        "MultitaskingViewFrame",               // Task View / Alt+Tab (Windows 10)
        "TaskListThumbnailWnd",                // taskbar thumbnails
        "ForegroundStaging",                   // shell focus hand-off helper
    ];
    SHELL_CLASSES.contains(&class)
}

pub fn is_pinnable(hwnd: Hwnd) -> bool {
    if !is_valid_window(hwnd) {
        return false;
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(h(hwnd), Some(&mut pid)) };
    if pid == unsafe { GetCurrentProcessId() } {
        return false; // PinIt's own windows
    }
    !is_shell_class(&class_name(hwnd))
}

/// Read physical pointer/button state on the interactive desktop. This stays
/// active while the mouse leaves PinIt's Slint window during a drag.
pub fn drag_mouse_down() -> bool {
    unsafe { GetAsyncKeyState(i32::from(VK_LBUTTON.0)) < 0 }
}

pub fn drag_cancelled_by_escape() -> bool {
    unsafe { GetAsyncKeyState(i32::from(VK_ESCAPE.0)) < 0 }
}

/// Keep the native crosshair visible even outside our Slint window. The
/// capture belongs to PinIt's UI thread and is held only for the mouse drag.
/// Never change the global/system cursor configuration with SetSystemCursor.
pub fn start_pin_drag_cursor(hwnd: Hwnd) {
    unsafe {
        if is_valid_window(hwnd) {
            SetCapture(h(hwnd));
            maintain_pin_drag_cursor();
        }
    }
}

/// Refresh after pointer movement; other window classes may reset the cursor
/// in WM_SETCURSOR, so keep asserting it only while actively dragging.
pub fn maintain_pin_drag_cursor() {
    unsafe {
        if let Ok(cross) = LoadCursorW(None, IDC_CROSS) {
            let _ = SetCursor(Some(cross));
        }
    }
}

pub fn stop_pin_drag_cursor(hwnd: Option<Hwnd>) {
    unsafe {
        if let Some(owner) = hwnd {
            if GetCapture() == h(owner) {
                let _ = ReleaseCapture();
            }
        }
        // The owner normally receives WM_SETCURSOR after capture changes;
        // explicitly reset it for stationary pointers as well.
        if let Ok(arrow) = LoadCursorW(None, IDC_ARROW) {
            let _ = SetCursor(Some(arrow));
        }
    }
}

/// Resolve a point on the virtual desktop to a visible, pinnable, top-level
/// window. Applying the same constraints as the picker prevents accidentally
/// pinning our overlay, PinIt, taskbars or shell surfaces.
/// Coordinates must be *physical screen pixels* (GetCursorPos).
pub fn window_at_point(x: i32, y: i32) -> Option<Hwnd> {
    unsafe {
        let found = WindowFromPoint(POINT { x, y });
        if found.0.is_null() {
            return None;
        }
        let hwnd = GetAncestor(found, GA_ROOT);
        if hwnd.0.is_null() {
            return None;
        }
        let id = hwnd.0 as Hwnd;
        if !is_pinnable(id) || !IsWindowVisible(hwnd).as_bool() {
            return None;
        }
        if GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0 {
            return None;
        }
        let mut cloaked = 0u32;
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut _ as *mut _,
            size_of::<u32>() as u32,
        ).is_ok() && cloaked != 0 {
            return None;
        }
        (!window_title(id).is_empty()).then_some(id)
    }
}

pub fn apply_topmost(hwnd: Hwnd) -> bool {
    unsafe {
        SetWindowPos(h(hwnd), Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE)
            .is_ok()
    }
}

pub fn remove_topmost(hwnd: Hwnd) -> bool {
    unsafe {
        SetWindowPos(h(hwnd), Some(HWND_NOTOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE)
            .is_ok()
    }
}

pub fn set_opacity_percent(hwnd: Hwnd, percent: i32) -> bool {
    let percent = percent.clamp(MIN_OPACITY, MAX_OPACITY);
    let ex = ex_style(hwnd);
    let added_layered = ex & WS_EX_LAYERED.0 == 0;
    // Keep an existing colour key; otherwise making the window translucent
    // would turn formerly transparent pixels into visible ones.
    let attrs = layered_attributes(hwnd);
    if added_layered && !set_ex_style(hwnd, ex | WS_EX_LAYERED.0) {
        return false;
    }
    let flags = if attrs.as_ref().is_some_and(|a| a.flags & LWA_COLORKEY.0 != 0) {
        LWA_ALPHA | LWA_COLORKEY
    } else {
        LWA_ALPHA
    };
    let colour = attrs.as_ref().map_or(0, |a| a.color_key);
    let success = unsafe {
        SetLayeredWindowAttributes(h(hwnd), COLORREF(colour), percent_to_alpha(percent), flags).is_ok()
    };
    // If modifying a normal window fails, leave it as a normal window.
    if !success && added_layered {
        let _ = set_ex_style(hwnd, ex);
    }
    success
}

pub fn set_click_through(hwnd: Hwnd, enabled: bool) -> bool {
    let ex = ex_style(hwnd);
    let wanted = if enabled { ex | WS_EX_TRANSPARENT.0 } else { ex & !WS_EX_TRANSPARENT.0 };
    if wanted != ex && !set_ex_style(hwnd, wanted) {
        return false;
    }
    is_click_through(hwnd) == enabled
}

/// Undo `set_opacity_percent`. `keep_layered`: the window was layered before
/// PinIt touched it (it manages its own transparency), so leave its style.
pub fn restore_opacity(hwnd: Hwnd, keep_layered: bool, original: Option<&LayeredAttributes>) -> bool {
    unsafe {
        if keep_layered {
            // Restore the original alpha and colour key, not just opacity.
            return original.is_some_and(|previous|
                SetLayeredWindowAttributes(
                    h(hwnd), COLORREF(previous.color_key), previous.alpha,
                    LAYERED_WINDOW_ATTRIBUTES_FLAGS(previous.flags),
                ).is_ok()
            );
        }
        let ex = ex_style(hwnd);
        if ex & WS_EX_LAYERED.0 == 0 {
            return true;
        }
        if !set_ex_style(hwnd, ex & !WS_EX_LAYERED.0) {
            return false;
        }
        SetWindowPos(
            h(hwnd), None, 0, 0, 0, 0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        ).is_ok()
    }
}

pub struct WindowInfo {
    pub hwnd: Hwnd,
    pub title: String,
    pub process: String,
}

/// Real application windows, front to back: visible, not cloaked, not tool
/// windows, not the shell, not PinIt.
pub fn enumerate_windows() -> Vec<WindowInfo> {
    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        unsafe {
            let out = &mut *(lparam.0 as *mut Vec<Hwnd>);
            if !IsWindowVisible(hwnd).as_bool() {
                return TRUE;
            }
            if GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0 {
                return TRUE;
            }
            let mut cloaked = 0u32;
            let cloak = DwmGetWindowAttribute(
                hwnd,
                DWMWA_CLOAKED,
                &mut cloaked as *mut _ as *mut _,
                size_of::<u32>() as u32,
            );
            if cloak.is_ok() && cloaked != 0 {
                return TRUE;
            }
            out.push(hwnd.0 as Hwnd);
            TRUE
        }
    }

    let mut handles: Vec<Hwnd> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(collect), LPARAM(&mut handles as *mut _ as isize));
    }
    handles
        .into_iter()
        .filter(|&hwnd| is_pinnable(hwnd))
        .filter_map(|hwnd| {
            let title = window_title(hwnd);
            (!title.is_empty()).then(|| WindowInfo { hwnd, title, process: process_name(hwnd) })
        })
        .collect()
}

/// The window's icon as premultiplied RGBA (`size` x `size`).
/// Prefer a native image at least as large as the requested size: the 16px
/// ICON_SMALL2 is often unsuitable for high-DPI UI. Never resize with GDI.
pub fn window_icon_rgba(hwnd: Hwnd, size: u32) -> Option<Vec<u8>> {
    if size == 0 {
        return None;
    }
    const TIMEOUT_MS: u32 = 100;
    unsafe {
        let mut handles = Vec::<HICON>::new();
        let mut push_unique = |raw: usize| {
            if raw != 0 && !handles.iter().any(|h| h.0 as usize == raw) {
                handles.push(HICON(raw as *mut _));
            }
        };

        // WM_GETICON may expose different *native* sizes for the same window.
        // Keep the timeout for hung applications and inspect all available
        // resolutions, including the class icons.
        for which in [ICON_BIG, ICON_SMALL2, ICON_SMALL] {
            let mut result = 0usize;
            let ok = SendMessageTimeoutW(
                h(hwnd),
                WM_GETICON,
                WPARAM(which as usize),
                LPARAM(0),
                SMTO_ABORTIFHUNG,
                TIMEOUT_MS,
                Some(&mut result),
            );
            if ok.0 != 0 {
                push_unique(result);
            }
        }
        push_unique(GetClassLongPtrW(h(hwnd), GCLP_HICON));
        push_unique(GetClassLongPtrW(h(hwnd), GCLP_HICONSM));
        drop(push_unique);

        // GetIconInfo reports the actual bitmap dimensions without DPI
        // virtualization. Prefer the nearest size that avoids upscaling;
        // if unavailable, take the largest small icon.
        let mut icons: Vec<_> = handles
            .into_iter()
            .map(|icon| (icon, icon_dimensions(icon)))
            .collect();
        icons.sort_by_key(|(_, dimensions)| crate::icon_resize::source_rank(*dimensions, size));

        for (icon, dimensions) in icons {
            // Ordinary ICO resources top out at 256px. Bound the DIB
            // allocation even for malformed or unusually large HICONs.
            let (width, height) = dimensions.unwrap_or((size, size));
            let (width, height) = (width.min(256), height.min(256));
            // Borrowed from another process: do not DestroyIcon.
            if let Some(native) = icon_to_rgba(icon, width, height) {
                if let Some(resized) =
                    crate::icon_resize::resize_premultiplied_rgba(&native, width, height, size)
                {
                    return Some(resized);
                }
            }
        }
        None
    }
}

/// Dimensions of the actual HICON bitmap, not the system's default icon
/// metrics. GetIconInfo allocates new GDI bitmaps, which we own and delete.
unsafe fn icon_dimensions(icon: HICON) -> Option<(u32, u32)> {
    unsafe {
        let mut info = ICONINFO::default();
        GetIconInfo(icon, &mut info).ok()?;
        let monochrome = info.hbmColor.0.is_null();
        let bitmap = if monochrome { info.hbmMask } else { info.hbmColor };
        let mut details = BITMAP::default();
        let bytes = if bitmap.0.is_null() {
            0
        } else {
            GetObjectW(
                bitmap.into(),
                size_of::<BITMAP>() as i32,
                Some(&mut details as *mut BITMAP as *mut core::ffi::c_void),
            )
        };
        if !info.hbmColor.0.is_null() {
            let _ = DeleteObject(info.hbmColor.into());
        }
        if !info.hbmMask.0.is_null() {
            let _ = DeleteObject(info.hbmMask.into());
        }
        if bytes == 0 || details.bmWidth <= 0 || details.bmHeight <= 0 {
            return None;
        }
        // A monochrome icon's mask stores AND and XOR planes vertically.
        let height = if monochrome { details.bmHeight / 2 } else { details.bmHeight };
        (height > 0).then_some((details.bmWidth as u32, height as u32))
    }
}

/// Rasterize the selected source at its *native* dimensions. Alpha is
/// premultiplied by DrawIconEx onto a transparent 32-bit DIB.
unsafe fn icon_to_rgba(icon: HICON, width: u32, height: u32) -> Option<Vec<u8>> {
    unsafe {
        let len = (width as usize).checked_mul(height as usize)?.checked_mul(4)?;
        let dc = CreateCompatibleDC(None);
        if dc.is_invalid() {
            return None;
        }
        let draw = |flags: DI_FLAGS, fill: u8| -> Option<Vec<u8>> {
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width as i32,
                    biHeight: -(height as i32), // top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let bmp = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
            if bits.is_null() {
                let _ = DeleteObject(bmp.into());
                return None;
            }
            std::ptr::write_bytes(bits as *mut u8, fill, len);
            let old = SelectObject(dc, bmp.into());
            let drawn = DrawIconEx(dc, 0, 0, icon, width as i32, height as i32, 0, None, flags);
            SelectObject(dc, old);
            let pixels = std::slice::from_raw_parts(bits as *const u8, len).to_vec();
            let _ = DeleteObject(bmp.into());
            drawn.ok()?;
            Some(pixels)
        };

        let result = (|| {
            let mut bgra = draw(DI_NORMAL, 0)?;
            // Legacy icons have no per-pixel alpha; recover it from the AND
            // mask rather than letting their opaque pixels vanish.
            if bgra.chunks_exact(4).all(|p| p[3] == 0) {
                let mask = draw(DI_MASK, 0xff)?;
                for (px, m) in bgra.chunks_exact_mut(4).zip(mask.chunks_exact(4)) {
                    px[3] = if m[0] == 0 { 255 } else { 0 };
                    if px[3] == 0 {
                        px[0] = 0;
                        px[1] = 0;
                        px[2] = 0;
                    }
                }
            }
            for px in bgra.chunks_exact_mut(4) {
                px.swap(0, 2); // BGRA -> premultiplied RGBA
            }
            Some(bgra)
        })();
        let _ = DeleteDC(dc);
        result
    }
}

/// Whether the user has Windows animations switched on (Settings >
/// Accessibility > Visual effects).
pub fn animations_enabled() -> bool {
    let mut enabled: BOOL = TRUE;
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some(&mut enabled as *mut _ as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
    }
    enabled.as_bool()
}

/// A soft bundled "tick" rather than the system ding. SND_ASYNC reads the
/// buffer after this returns, which a `'static` slice allows.
pub fn play_pin_sound() {
    static TICK: &[u8] = include_bytes!("../../resources/tick.wav");
    unsafe {
        let _ = PlaySoundW(PCWSTR(TICK.as_ptr() as *const u16), None, SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
    }
}

/// Make PinIt's own title bar match its theme: dark caption buttons in dark
/// mode, and (Windows 11) the caption painted in the window's own colour.
pub fn style_title_bar(hwnd: Hwnd, dark: bool, caption_rgb: u32) {
    unsafe {
        let dark_flag: BOOL = dark.into();
        let _ = DwmSetWindowAttribute(
            h(hwnd),
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark_flag as *const _ as *const _,
            size_of::<BOOL>() as u32,
        );
        // COLORREF is 0x00BBGGRR.
        let (r, g, b) = ((caption_rgb >> 16) & 0xff, (caption_rgb >> 8) & 0xff, caption_rgb & 0xff);
        let color = COLORREF(r | (g << 8) | (b << 16));
        let _ = DwmSetWindowAttribute(
            h(hwnd),
            DWMWA_CAPTION_COLOR,
            &color as *const _ as *const _,
            size_of::<COLORREF>() as u32,
        );
    }
}

/// Bring one of PinIt's own windows to the front, even when another app has
/// the focus (a second launch, the tray, the show/hide hotkey).
pub fn force_foreground(hwnd: Hwnd) {
    unsafe {
        if IsIconic(h(hwnd)).as_bool() {
            let _ = ShowWindow(h(hwnd), SW_RESTORE);
        }
        let _ = SetForegroundWindow(h(hwnd));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opacity_round_trips() {
        for p in 0..=100 {
            assert_eq!(alpha_to_percent(percent_to_alpha(p) as i32), p);
        }
        assert_eq!(percent_to_alpha(100), 255);
        assert_eq!(percent_to_alpha(150), 255);
        assert_eq!(alpha_to_percent(-3), 0);
    }

    #[test]
    fn shell_classes_are_not_pinnable() {
        assert!(is_shell_class("Shell_TrayWnd"));
        assert!(is_shell_class("Progman"));
        assert!(!is_shell_class("Notepad"));
    }
}
