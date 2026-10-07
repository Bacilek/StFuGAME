//! Notification area icon (next to the clock): start and stop the bot, open the log, exit.
//! Green = bot running, grey = stopped, red = ended with an error (see the log).
//!
//! The only place with `unsafe`: Windows API calls (like P/Invoke in C#) for the message loop
//! the icon needs, and for making sure only one instance runs.

use std::{future::Future, process::ExitCode, time::Duration};

use tokio::{runtime::Runtime, task::JoinHandle};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};
use windows_sys::Win32::{
    Foundation::{ERROR_ALREADY_EXISTS, GetLastError},
    System::Threading::CreateMutexW,
    UI::WindowsAndMessaging::{DispatchMessageW, MB_ICONINFORMATION, MSG, MessageBoxW, PM_REMOVE, PeekMessageW, TranslateMessage},
};

#[derive(Clone, Copy, PartialEq)]
enum State {
    Running,
    Stopped,
    Failed,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Is another instance of the bot already running? (named Windows mutex, held until the process ends)
pub fn already_running() -> bool {
    let name = wide("Local\\StFuGAME_bot");
    // SAFETY: valid null-terminated UTF-16 string; the handle is deliberately not closed (holds the mutex until exit)
    unsafe {
        CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        GetLastError() == ERROR_ALREADY_EXISTS
    }
}

/// Information dialog (e.g. "bot already running").
pub fn message_box(text: &str) {
    let (text, caption) = (wide(text), wide("StFuGAME"));
    // SAFETY: valid null-terminated UTF-16 strings, no parent window
    unsafe {
        MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), MB_ICONINFORMATION);
    }
}

/// Round 32×32 icon in the given colour.
fn icon(state: State) -> Icon {
    let (r, g, b) = match state {
        State::Running => (46, 160, 67),
        State::Stopped => (140, 140, 140),
        State::Failed => (200, 50, 50),
    };
    let size = 32u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    let c = (size as f32 - 1.0) / 2.0;
    for y in 0..size {
        for x in 0..size {
            let d = ((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt();
            let alpha = if d <= c - 1.0 { 255 } else if d <= c { 128 } else { 0 };
            rgba.extend_from_slice(&[r, g, b, alpha]);
        }
    }
    Icon::from_rgba(rgba, size, size).expect("icon from valid RGBA data")
}

fn set_state(tray: &TrayIcon, start: &MenuItem, stop: &MenuItem, state: State) {
    let tip = match state {
        State::Running => "StFuGAME: bot running",
        State::Stopped => "StFuGAME: bot stopped",
        State::Failed => "StFuGAME: bot ended with an error (see log)",
    };
    let _ = tray.set_icon(Some(icon(state)));
    let _ = tray.set_tooltip(Some(tip));
    start.set_enabled(state != State::Running);
    stop.set_enabled(state == State::Running);
}

/// Starts the icon and right away the bot. Blocks until the user picks "Exit".
pub fn run<F, Fut>(rt: &Runtime, bot: F)
where
    F: Fn() -> Fut,
    Fut: Future<Output = ExitCode> + Send + 'static,
{
    let menu = Menu::new();
    let start = MenuItem::new("Start bot", false, None);
    let stop = MenuItem::new("Stop bot", true, None);
    let log = MenuItem::new("Open log", true, None);
    let quit = MenuItem::new("Exit", true, None);
    let _ = menu.append_items(&[&start, &stop, &PredefinedMenuItem::separator(), &log, &PredefinedMenuItem::separator(), &quit]);

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon(State::Running))
        .with_tooltip("StFuGAME: bot running")
        .build()
        .expect("creating the notification area icon");

    let mut handle: Option<JoinHandle<ExitCode>> = Some(rt.spawn(bot()));
    report!("[control] Bot started");
    let mut state = State::Running;
    set_state(&tray, &start, &stop, state);

    loop {
        // Windows message loop (the icon does not respond without it)
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        // SAFETY: standard message loop over a valid MSG structure
        unsafe {
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == start.id() && state != State::Running {
                handle = Some(rt.spawn(bot()));
                report!("[control] Bot started");
                state = State::Running;
                set_state(&tray, &start, &stop, state);
            } else if event.id == stop.id() && state == State::Running {
                if let Some(h) = handle.take() {
                    h.abort();
                }
                report!("[control] Bot stopped");
                state = State::Stopped;
                set_state(&tray, &start, &stop, state);
            } else if event.id == log.id() {
                let _ = std::process::Command::new("explorer").arg("logs\\progress.log").spawn();
            } else if event.id == quit.id() {
                if let Some(h) = handle.take() {
                    h.abort();
                }
                report!("[control] Exited");
                return;
            }
        }

        // The bot ended by itself (login error etc.)
        if state == State::Running && handle.as_ref().is_some_and(JoinHandle::is_finished) {
            handle = None;
            report!("[control] The bot ended by itself, details in the log");
            state = State::Failed;
            set_state(&tray, &start, &stop, state);
        }

        std::thread::sleep(Duration::from_millis(50));
    }
}
