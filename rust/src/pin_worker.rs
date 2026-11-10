//! Foreign windows can stop dispatching messages. Keep their synchronous
//! Win32 calls on one serial worker, and publish completed state to the UI.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, atomic::{AtomicBool, Ordering}, mpsc};

use crate::pins::{PinError, Pinned, Pins, Reenforced};
use crate::store::SavedPin;
use crate::win::{self, Hwnd};

#[derive(Clone, Default)]
pub struct Snapshot(Pins);

impl Snapshot {
    pub fn pinned(&self) -> &[Pinned] { self.0.pinned() }
    pub fn pending(&self) -> &[SavedPin] { self.0.pending() }
    pub fn is_pinned(&self, hwnd: Hwnd) -> bool { self.0.is_pinned(hwnd) }
    pub fn is_active(&self) -> bool { self.0.is_active() }
    pub fn saved(&self) -> Vec<SavedPin> { self.0.saved() }
}

pub enum Command {
    Restore(Vec<SavedPin>),
    Toggle { hwnd: Hwnd, from_hotkey: bool },
    Pin(Hwnd),
    SetOpacity { hwnd: Hwnd, percent: i32 },
    AdjustOpacity { hwnd: Hwnd, delta: i32 },
    ToggleClickThrough(Hwnd),
    Remove(Hwnd),
    UnpinAll,
    ToggleWaiting(SavedPin),
    RemoveWaiting(SavedPin),
    Reenforce,
}

pub enum Outcome {
    Restored,
    Toggled { hwnd: Hwnd, title: String, from_hotkey: bool, result: Result<bool, PinError> },
    Opacity { from_hotkey: bool, result: Result<i32, PinError> },
    ClickThrough(Result<bool, PinError>),
    Removed { restored: bool },
    UnpinnedAll { count: usize, failures: usize },
    WaitingChanged,
    Reenforced(Reenforced),
}

pub enum Completion {
    Updated { snapshot: Snapshot, outcome: Outcome },
    ShutdownFinished,
}

#[derive(Default)]
struct Queue {
    commands: VecDeque<Command>,
    maintenance_pending: bool,
    active: bool,
    closing: bool,
    cleanup_done: bool,
}

impl Queue {
    fn push(&mut self, command: Command) {
        if matches!(command, Command::Reenforce) {
            if self.maintenance_pending { return; }
            self.maintenance_pending = true;
        }
        // Consecutive slider updates need only their latest value. Do not
        // coalesce across another command: removal/toggle ordering matters.
        if let (Some(Command::SetOpacity { hwnd: last, percent }), Command::SetOpacity { hwnd, percent: next }) =
            (self.commands.back_mut(), &command)
        {
            if last == hwnd {
                *percent = *next;
                return;
            }
        }
        self.commands.push_back(command);
    }
}

struct Shared {
    queue: Mutex<Queue>,
    available: Condvar,
    latest: Mutex<Pins>,
    recovery: Mutex<Pins>,
    stopping: Arc<AtomicBool>,
    session_ending: AtomicBool,
}

pub struct PinWorker {
    shared: Arc<Shared>,
    results: mpsc::Receiver<Completion>,
    sender: mpsc::Sender<Completion>,
    notify: fn(),
}

impl PinWorker {
    pub fn new(saved: Vec<SavedPin>, notify: fn()) -> Self {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::default()),
            available: Condvar::new(),
            latest: Mutex::new(Pins::waiting(saved.clone())),
            recovery: Mutex::new(Pins::default()),
            stopping: Arc::new(AtomicBool::new(false)),
            session_ending: AtomicBool::new(false),
        });
        let (sender, results) = mpsc::channel();
        let worker_shared = shared.clone();
        let worker_sender = sender.clone();
        std::thread::Builder::new().name("pinit-pins".into()).spawn(move || {
            run(worker_shared, worker_sender, notify);
        }).expect("could not start the window worker");
        let worker = Self { shared, results, sender, notify };
        worker.submit(Command::Restore(saved));
        worker
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot(self.shared.latest.lock().unwrap().clone())
    }

    pub fn submit(&self, command: Command) {
        let mut queue = self.shared.queue.lock().unwrap();
        if queue.closing { return; }
        queue.push(command);
        self.shared.available.notify_one();
    }

    pub fn next_result(&self) -> Option<Completion> {
        self.results.try_recv().ok()
    }

    pub fn set_session_ending(&self, ending: bool) {
        self.shared.session_ending.store(ending, Ordering::Release);
    }

    /// At most one extra worker exists during shutdown. It releases known
    /// healthy windows even if the normal worker is stuck in another app.
    /// The UI owns the deadline and never joins either thread.
    pub fn shutdown(&self) {
        {
            let mut queue = self.shared.queue.lock().unwrap();
            if queue.closing { return; }
            self.shared.stopping.store(true, Ordering::Release);
            queue.closing = true;
            queue.commands.clear();
            self.shared.available.notify_one();
        }
        let pins = self.shared.recovery.lock().unwrap().clone();
        let shared = self.shared.clone();
        let sender = self.sender.clone();
        let notify = self.notify;
        std::thread::Builder::new().name("pinit-cleanup".into()).spawn(move || {
            pins.release_for_exit();
            let finished = {
                let mut queue = shared.queue.lock().unwrap();
                queue.cleanup_done = true;
                !queue.active
            };
            if finished { send(&sender, notify, Completion::ShutdownFinished); }
        }).expect("could not start window cleanup");
    }
}

fn send(sender: &mpsc::Sender<Completion>, notify: fn(), result: Completion) {
    if sender.send(result).is_ok() { notify(); }
}

fn run(shared: Arc<Shared>, sender: mpsc::Sender<Completion>, notify: fn()) {
    let mut pins = Pins::default();
    let weak = Arc::downgrade(&shared);
    pins.set_worker_hooks(shared.stopping.clone(), move |pins| {
        if let Some(shared) = weak.upgrade() {
            *shared.recovery.lock().unwrap() = pins.clone();
        }
    });
    loop {
        let command = {
            let mut queue = shared.queue.lock().unwrap();
            while queue.commands.is_empty() && !queue.closing {
                queue = shared.available.wait(queue).unwrap();
            }
            if queue.closing { return; }
            queue.active = true;
            queue.commands.pop_front().unwrap()
        };
        let maintenance = matches!(command, Command::Reenforce);
        pins.set_session_ending(shared.session_ending.load(Ordering::Acquire));
        let outcome = execute(&mut pins, command);
        *shared.latest.lock().unwrap() = pins.clone();
        *shared.recovery.lock().unwrap() = pins.clone();
        let closing = {
            let mut queue = shared.queue.lock().unwrap();
            if maintenance { queue.maintenance_pending = false; }
            if !queue.closing { queue.active = false; }
            queue.closing
        };
        if closing {
            // A pin may have been added while Quit was requested, before it
            // reached the UI snapshot. Restore the actual completed state too.
            pins.release_for_exit();
            let finished = {
                let mut queue = shared.queue.lock().unwrap();
                queue.active = false;
                queue.cleanup_done
            };
            if finished { send(&sender, notify, Completion::ShutdownFinished); }
            return;
        }
        send(&sender, notify, Completion::Updated { snapshot: Snapshot(pins.clone()), outcome });
    }
}

fn execute(pins: &mut Pins, command: Command) -> Outcome {
    match command {
        Command::Restore(saved) => {
            pins.restore(saved);
            Outcome::Restored
        }
        Command::Toggle { hwnd, from_hotkey } => {
            let title = win::window_title(hwnd);
            Outcome::Toggled { hwnd, title, from_hotkey, result: pins.toggle(hwnd) }
        }
        Command::Pin(hwnd) => {
            let title = win::window_title(hwnd);
            Outcome::Toggled { hwnd, title, from_hotkey: false, result: pins.pin(hwnd).map(|()| true) }
        }
        Command::SetOpacity { hwnd, percent } =>
            Outcome::Opacity { from_hotkey: false, result: pins.set_opacity(hwnd, percent) },
        Command::AdjustOpacity { hwnd, delta } =>
            Outcome::Opacity { from_hotkey: true, result: pins.adjust_opacity(hwnd, delta) },
        Command::ToggleClickThrough(hwnd) => {
            let result = match pins.pinned().iter().find(|p| p.hwnd == hwnd) {
                Some(p) => {
                    let enabled = !p.click_through;
                    pins.set_click_through(hwnd, enabled).map(|()| enabled)
                }
                None => Err(PinError::NotPinned),
            };
            Outcome::ClickThrough(result)
        }
        Command::Remove(hwnd) =>
            Outcome::Removed { restored: pins.unpin(hwnd).is_none_or(|(_, restored)| restored) },
        Command::UnpinAll => {
            let (count, failures) = pins.unpin_all();
            Outcome::UnpinnedAll { count, failures }
        }
        Command::ToggleWaiting(saved) => {
            // Identify the selected saved record, not an index that a queued
            // discovery pass may have shifted in the meantime.
            if let Some(i) = pins.pending().iter().position(|p| same_waiting_record(p, &saved)) {
                pins.toggle_pending(i);
            } else if let Some(hwnd) = pins.pinned().iter()
                .find(|p| saved.matches(&p.process, &p.title)).map(|p| p.hwnd)
            {
                return Outcome::Toggled { hwnd, title: saved.title,
                    from_hotkey: false, result: pins.toggle(hwnd) };
            }
            Outcome::WaitingChanged
        }
        Command::RemoveWaiting(saved) => {
            if let Some(i) = pins.pending().iter().position(|p| same_waiting_record(p, &saved)) {
                pins.forget_pending(i);
            } else if let Some(hwnd) = pins.pinned().iter()
                .find(|p| saved.matches(&p.process, &p.title)).map(|p| p.hwnd)
            {
                return Outcome::Removed { restored: pins.unpin(hwnd).is_none_or(|(_, restored)| restored) };
            }
            Outcome::WaitingChanged
        }
        Command::Reenforce => Outcome::Reenforced(pins.reenforce()),
    }
}

// The UI can enqueue a second click before the first toggle completes. The
// enabled flag is mutable state, not the identity of that waiting record.
fn same_waiting_record(a: &SavedPin, b: &SavedPin) -> bool {
    a.process_name == b.process_name && a.title == b.title
        && a.opacity == b.opacity && a.click_through == b.click_through
        && a.was_layered == b.was_layered && a.original_layered == b.original_layered
        && a.was_topmost == b.was_topmost && a.was_click_through == b.was_click_through
        && a.extra == b.extra
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalescing_preserves_user_command_order() {
        let mut queue = Queue::default();
        queue.push(Command::SetOpacity { hwnd: 1, percent: 30 });
        queue.push(Command::SetOpacity { hwnd: 1, percent: 45 });
        queue.push(Command::ToggleClickThrough(1));
        queue.push(Command::SetOpacity { hwnd: 1, percent: 60 });
        assert_eq!(queue.commands.len(), 3);
        assert!(matches!(queue.commands.pop_front(), Some(Command::SetOpacity { percent: 45, .. })));
        assert!(matches!(queue.commands.pop_front(), Some(Command::ToggleClickThrough(1))));
        assert!(matches!(queue.commands.pop_front(), Some(Command::SetOpacity { percent: 60, .. })));
    }

    #[test]
    fn repeated_foreground_events_do_not_fill_the_worker_queue() {
        let mut queue = Queue::default();
        for _ in 0..100 { queue.push(Command::Reenforce); }
        assert_eq!(queue.commands.len(), 1);
        queue.commands.pop_front();
        queue.push(Command::Reenforce);
        assert!(queue.commands.is_empty(), "an in-flight maintenance pass is also coalesced");
    }

    #[test]
    fn queued_waiting_clicks_survive_the_first_toggle() {
        let saved: SavedPin = serde_json::from_str(
            r#"{"process_name":"Code.exe","title":"Notes"}"#,
        ).unwrap();
        let mut pins = Pins::waiting(vec![saved.clone()]);
        execute(&mut pins, Command::ToggleWaiting(saved.clone()));
        assert!(!pins.pending()[0].enabled);
        execute(&mut pins, Command::ToggleWaiting(saved.clone()));
        assert!(pins.pending()[0].enabled);
        execute(&mut pins, Command::ToggleWaiting(saved.clone()));
        execute(&mut pins, Command::RemoveWaiting(saved));
        assert!(pins.pending().is_empty());
    }
}
