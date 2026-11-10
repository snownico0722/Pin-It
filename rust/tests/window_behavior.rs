//! Runs the real executable against a real Win32 window on an interactive desktop.
//! No PinIt APIs are mocked. Keep these scenarios sequential: they share foreground focus.
#![cfg(windows)]

use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use windows::core::{BOOL, HSTRING, PCWSTR, w};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
use windows::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEINPUT, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP};
use windows::Win32::UI::WindowsAndMessaging::*;

const STALL: u32 = WM_APP + 20;
static STALL_TARGET: AtomicUsize = AtomicUsize::new(0);
static TARGET_STALLED: AtomicBool = AtomicBool::new(false);
static RELEASE_TARGET: AtomicBool = AtomicBool::new(false);
static DEMOTE_SUCCEEDED: AtomicBool = AtomicBool::new(false);
static WINDOWPOS_COUNT: AtomicUsize = AtomicUsize::new(0);
static LAST_INSERT_AFTER: AtomicIsize = AtomicIsize::new(0);
static LAST_WINDOWPOS_FLAGS: AtomicU32 = AtomicU32::new(0);

fn wait_for(description: &str, timeout: Duration, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + timeout;
    while !condition() {
        assert!(Instant::now() < deadline, "Timed out: {description}");
        thread::sleep(Duration::from_millis(15));
    }
}

fn post(hwnd: HWND, message: u32, value: usize) {
    unsafe { PostMessageW(Some(hwnd), message, WPARAM(value), LPARAM(0)).unwrap(); }
}

fn topmost(hwnd: HWND) -> bool {
    unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST.0 != 0 }
}

fn bounds(hwnd: HWND) -> RECT {
    let mut rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rect).unwrap(); }
    rect
}

fn width(hwnd: HWND) -> i32 {
    let rect = bounds(hwnd);
    rect.right - rect.left
}

fn click_brand(hwnd: HWND, compact: bool) {
    let rect = bounds(hwnd);
    let scale = (rect.right - rect.left) as f64 / if compact { 144.0 } else { 372.0 };
    let (x, y) = if compact { (72.0, 36.0) } else { (55.0, 41.0) };
    unsafe {
        SetCursorPos(rect.left + (x * scale) as i32, rect.top + (y * scale) as i32).unwrap();
        let mouse = |flags| INPUT { r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 { mi: MOUSEINPUT { dwFlags: flags, ..Default::default() } } };
        let down = SendInput(&[mouse(MOUSEEVENTF_LEFTDOWN)], std::mem::size_of::<INPUT>() as i32);
        thread::sleep(Duration::from_millis(50));
        let up = SendInput(&[mouse(MOUSEEVENTF_LEFTUP)], std::mem::size_of::<INPUT>() as i32);
        assert_eq!((down, up), (1, 1), "Interactive mouse input is required for the mini-window check");
    }
}

fn windows_in_order() -> Vec<HWND> {
    unsafe extern "system" fn collect(hwnd: HWND, data: LPARAM) -> BOOL {
        unsafe { (*(data.0 as *mut Vec<HWND>)).push(hwnd); }
        BOOL(1)
    }
    let mut windows = Vec::new();
    unsafe { let _ = EnumWindows(Some(collect), LPARAM(&mut windows as *mut _ as isize)); }
    windows
}

fn main_window(pid: u32) -> Option<HWND> {
    windows_in_order().into_iter().find(|&hwnd| unsafe {
        let mut owner = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut owner));
        let mut title = [0u16; 128];
        let count = GetWindowTextW(hwnd, &mut title);
        owner == pid && GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 == 0
            && String::from_utf16_lossy(&title[..count.max(0) as usize]) == "PinIt"
    })
}

fn above(front: HWND, back: HWND) -> bool {
    let order = windows_in_order();
    matches!((order.iter().position(|&h| h == front), order.iter().position(|&h| h == back)),
        (Some(a), Some(b)) if a < b)
}

unsafe extern "system" fn target_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            STALL => {
                // Make PinIt's next real maintenance pass reapply topmost.
                // Put it behind responsive desktop windows as well, so hiding
                // PinIt does not ask Windows to activate the deliberately hung target.
                // Arm before SetWindowPos: it can dispatch a waiting cross-thread
                // repin before returning, so arming afterwards can miss that call.
                STALL_TARGET.store(hwnd.0 as usize, Ordering::SeqCst);
                let demoted = SetWindowPos(hwnd, Some(HWND_BOTTOM), 0, 0, 0, 0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE).is_ok();
                DEMOTE_SUCCEEDED.store(demoted, Ordering::SeqCst);
                LRESULT(0)
            }
            WM_WINDOWPOSCHANGING if STALL_TARGET.load(Ordering::SeqCst) == hwnd.0 as usize => {
                let position = &*(lp.0 as *const WINDOWPOS);
                LAST_INSERT_AFTER.store(position.hwndInsertAfter.0 as isize, Ordering::SeqCst);
                LAST_WINDOWPOS_FLAGS.store(position.flags.0, Ordering::SeqCst);
                WINDOWPOS_COUNT.fetch_add(1, Ordering::SeqCst);
                if position.hwndInsertAfter == HWND_TOPMOST {
                    // Signal only once the real cross-process topmost operation
                    // reaches this WndProc; elapsed time alone does not prove it.
                    TARGET_STALLED.store(true, Ordering::SeqCst);
                    while !RELEASE_TARGET.load(Ordering::SeqCst) {
                        thread::sleep(Duration::from_millis(10));
                    }
                }
                DefWindowProcW(hwnd, msg, wp, lp)
            }
            WM_DESTROY => { PostQuitMessage(0); LRESULT(0) }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
}

struct TargetWindow { hwnd: HWND, thread: Option<thread::JoinHandle<()>> }

impl TargetWindow {
    fn create(number: u32) -> Self {
        let (send, receive) = mpsc::sync_channel(1);
        let thread = thread::spawn(move || unsafe {
            let instance = GetModuleHandleW(None).unwrap();
            let class = HSTRING::from(format!("PinItBehaviorTarget{}-{number}", std::process::id()));
            let wc = WNDCLASSW {
                lpfnWndProc: Some(target_proc), hInstance: instance.into(),
                lpszClassName: PCWSTR(class.as_ptr()), ..Default::default()
            };
            assert_ne!(RegisterClassW(&wc), 0);
            let hwnd = CreateWindowExW(WINDOW_EX_STYLE(0), &class, w!("PinIt behavior target"),
                WS_OVERLAPPEDWINDOW, 40, 40, 800, 700, None, None, Some(instance.into()), None).unwrap();
            let _ = ShowWindow(hwnd, SW_SHOW);
            send.send(hwnd.0 as usize).unwrap();
            let mut message = MSG::default();
            while GetMessageW(&mut message, None, 0, 0).0 > 0 {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        });
        let hwnd = HWND(receive.recv_timeout(Duration::from_secs(5)).unwrap() as *mut _);
        Self { hwnd, thread: Some(thread) }
    }

    fn activate(&self) {
        // This is test setup only. PinIt's own activation is tested through the
        // real second-launch handoff below, without attaching input queues.
        let accepted = unsafe { SetForegroundWindow(self.hwnd).as_bool() };
        assert!(accepted || unsafe { GetForegroundWindow() == self.hwnd },
            "An interactive desktop allowing foreground changes is required");
    }
}

impl Drop for TargetWindow {
    fn drop(&mut self) {
        if thread::panicking() && !RELEASE_TARGET.load(Ordering::SeqCst) {
            let armed = STALL_TARGET.load(Ordering::SeqCst);
            eprintln!("Stall probe: armed={armed:#x}, demote_ok={}, stalled={}, target_topmost={}, windowpos_count={}, last_insert_after={:#x}, last_flags={:#x}",
                DEMOTE_SUCCEEDED.load(Ordering::SeqCst), TARGET_STALLED.load(Ordering::SeqCst),
                armed != 0 && topmost(HWND(armed as *mut _)), WINDOWPOS_COUNT.load(Ordering::SeqCst),
                LAST_INSERT_AFTER.load(Ordering::SeqCst), LAST_WINDOWPOS_FLAGS.load(Ordering::SeqCst));
        }
        RELEASE_TARGET.store(true, Ordering::SeqCst);
        unsafe { let _ = PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)); }
        if let Some(thread) = self.thread.take() { let _ = thread.join(); }
    }
}

struct PinItProcess { child: Child, directory: PathBuf }

impl PinItProcess {
    fn start() -> Self {
        assert!(unsafe { FindWindowW(w!("PinItHost-dev"), PCWSTR::null()).is_err() },
            "An existing PinIt-dev instance is running; this test will not use or close it");
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let directory = std::env::temp_dir().join(format!("pinit-behavior-{}-{nonce}", std::process::id()));
        std::fs::create_dir_all(directory.join("PinIt-dev")).unwrap();
        std::fs::write(directory.join("PinIt-dev/pinned.json"), r#"{"settings":{
            "pin_when_focused":true,"keep_absolute_topmost":true,"aggressive_mode":false,
            "exit_on_close":true,"enable_sound":false,"show_notifications":false,
            "has_seen_tray_notice":true,"ui_language":"en"}}"#).unwrap();
        let child = Self::command(&directory).spawn().unwrap();
        Self { child, directory }
    }

    fn command(directory: &std::path::Path) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_pinit"));
        command.env("PINIT_DEV", "1").env("LOCALAPPDATA", directory);
        command
    }

    fn wake(&self, hwnd: HWND) {
        let mut handoff = Self::command(&self.directory).spawn().unwrap();
        let exited = wait_exit(&mut handoff, Duration::from_secs(3));
        if exited.is_none() { let _ = handoff.kill(); let _ = handoff.wait(); }
        assert!(exited.is_some_and(|s| s.success()), "Second-launch handoff did not exit successfully");
        wait_for("second launch must restore and activate PinIt", Duration::from_secs(3), || unsafe {
            IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool() && GetForegroundWindow() == hwnd
        });
    }
}

fn wait_exit(child: &mut Child, timeout: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().unwrap() { return Some(status); }
        if Instant::now() >= deadline { return None; }
        thread::sleep(Duration::from_millis(15));
    }
}

impl Drop for PinItProcess {
    fn drop(&mut self) {
        if thread::panicking() {
            if let Ok(log) = std::fs::read_to_string(self.directory.join("PinIt-dev/pinit.log")) {
                eprintln!("PinIt log:\n{log}");
            }
        }
        // Only this test's child is terminated. Never find/kill arbitrary PinIt processes.
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn foreground_wake_and_stalled_target_use_real_windows() {
    unsafe { let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2); }
    let target = TargetWindow::create(0);
    // PinIt has not started: prove the OS itself preserves native TOPMOST when
    // a backend rewrites extended styles, without any PinIt subclass helping it.
    unsafe {
        let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
        SetWindowPos(target.hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, flags).unwrap();
        let styles = GetWindowLongW(target.hwnd, GWL_EXSTYLE);
        SetWindowLongW(target.hwnd, GWL_EXSTYLE, styles & !(WS_EX_TOPMOST.0 as i32));
        SetWindowPos(target.hwnd, None, 0, 0, 0, 0, flags | SWP_NOZORDER | SWP_FRAMECHANGED).unwrap();
        assert!(topmost(target.hwnd), "A style refresh must not erase native TOPMOST");
        SetWindowPos(target.hwnd, Some(HWND_NOTOPMOST), 0, 0, 0, 0, flags).unwrap();
    }
    let mut app = PinItProcess::start();
    wait_for("PinIt main window", Duration::from_secs(10), || main_window(app.child.id()).is_some());
    let hwnd = main_window(app.child.id()).unwrap();
    let host = unsafe { FindWindowW(w!("PinItHost-dev"), PCWSTR::null()).unwrap() };
    unsafe { SetWindowPos(hwnd, None, 170, 150, 0, 0,
        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE).unwrap(); }
    app.wake(hwnd);

    // 1. Focus-only pinning must yield to a normal, overlapping foreground window.
    wait_for("focused PinIt must be topmost", Duration::from_secs(2), || topmost(hwnd));
    let full_width = width(hwnd);
    click_brand(hwnd, false);
    wait_for("brand click must enter mini mode", Duration::from_secs(2),
        || width(hwnd) < full_width / 2);
    target.activate();
    thread::sleep(Duration::from_millis(150));
    assert!(topmost(hwnd), "An unfocused mini window must stay topmost");
    assert_eq!(unsafe { GetForegroundWindow() }, target.hwnd, "Mini mode stole keyboard focus");
    app.wake(hwnd);
    click_brand(hwnd, true);
    wait_for("mini click must restore the full window", Duration::from_secs(2),
        || (width(hwnd) - full_width).abs() <= 2);
    target.activate();
    wait_for("unfocused PinIt must yield topmost and Z-order", Duration::from_secs(2),
        || !topmost(hwnd) && above(target.hwnd, hwnd));
    let until = Instant::now() + Duration::from_millis(600);
    while Instant::now() < until {
        assert_eq!(unsafe { GetForegroundWindow() }, target.hwnd,
            "Changing PinIt's topmost state reactivated it after focus left");
        assert!(above(target.hwnd, hwnd),
            "Unfocused PinIt still covers the normal foreground window");
        thread::sleep(Duration::from_millis(15));
    }

    // 2. Exercise the real hotkey-hide, second-instance and native minimize paths.
    app.wake(hwnd);
    post(host, WM_HOTKEY, 4);
    wait_for("hotkey must hide PinIt", Duration::from_secs(2), || unsafe { !IsWindowVisible(hwnd).as_bool() });
    app.wake(hwnd);
    post(hwnd, WM_SYSCOMMAND, SC_MINIMIZE as usize);
    wait_for("native minimize", Duration::from_secs(2), || unsafe { IsIconic(hwnd).as_bool() });
    thread::sleep(Duration::from_millis(300));
    assert!(unsafe { IsIconic(hwnd).as_bool() }, "Topmost maintenance restored a minimized window");
    app.wake(hwnd);

    // 3. A managed foreign window stops dispatching messages. PinIt's event loop
    // must still hide, wake and exit while that target remains unresponsive.
    target.activate();
    post(host, WM_HOTKEY, 1);
    wait_for("real target must be pinned", Duration::from_secs(3), || topmost(target.hwnd));
    let healthy = TargetWindow::create(1);
    healthy.activate();
    post(host, WM_HOTKEY, 1);
    wait_for("healthy target must be pinned", Duration::from_secs(3), || topmost(healthy.hwnd));
    app.wake(hwnd);
    post(target.hwnd, STALL, 0);
    wait_for("real maintenance must reach the target's stalled WndProc", Duration::from_secs(5),
        || TARGET_STALLED.load(Ordering::SeqCst));
    assert!(!topmost(target.hwnd));
    post(host, WM_HOTKEY, 4);
    wait_for("PinIt must hide while target is stalled", Duration::from_secs(2), || unsafe { !IsWindowVisible(hwnd).as_bool() });
    app.wake(hwnd);
    post(hwnd, WM_CLOSE, 0);
    let status = wait_exit(&mut app.child, Duration::from_secs(3));
    assert!(status.is_some_and(|s| s.success()), "Exit waited indefinitely for the stalled foreign window");
    wait_for("quit must also unpin the responsive target", Duration::from_secs(2), || !topmost(healthy.hwnd));
    assert!(!RELEASE_TARGET.load(Ordering::SeqCst), "The target must stay stalled throughout the responsiveness checks");
}
