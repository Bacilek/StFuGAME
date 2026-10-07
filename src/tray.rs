//! Notification area icon (next to the clock): global start/stop, open the log, open the dashboard, exit.
//! Green = at least one character running, grey = all stopped, red = all ended with an error (see the log).
//! The actual window (tiles, per-character start/stop, charts) is built in `app.rs`; this module only owns
//! the icon and its menu, which `app.rs` polls from the same event loop.
//!
//! The only place with `unsafe`: Windows API calls (like P/Invoke in C#) for making sure only one instance runs.

use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuItem, PredefinedMenuItem},
};
use windows_sys::Win32::{
    Foundation::{ERROR_ALREADY_EXISTS, GetLastError},
    System::Threading::CreateMutexW,
    UI::WindowsAndMessaging::{MB_ICONINFORMATION, MessageBoxW},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum State {
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

/// The icon plus its menu items, built once by `app.rs`.
pub struct Tray {
    pub tray: TrayIcon,
    pub start: MenuItem,
    pub stop: MenuItem,
    pub log: MenuItem,
    pub dash: MenuItem,
    pub eod: MenuItem,
    pub show: MenuItem,
    pub quit: MenuItem,
}

impl Tray {
    pub fn build() -> Self {
        let menu = Menu::new();
        let show = MenuItem::new("Open StFuGAME", true, None);
        let start = MenuItem::new("Start all characters", true, None);
        let stop = MenuItem::new("Stop all characters", true, None);
        let log = MenuItem::new("Open log", true, None);
        let dash = MenuItem::new("Open dashboard in browser", true, None);
        let eod = MenuItem::new("Run end of day now (preview)", true, None);
        let quit = MenuItem::new("Exit", true, None);
        let _ = menu.append_items(&[
            &show,
            &PredefinedMenuItem::separator(),
            &start,
            &stop,
            &PredefinedMenuItem::separator(),
            &log,
            &dash,
            &eod,
            &PredefinedMenuItem::separator(),
            &quit,
        ]);
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(icon(State::Running))
            .with_tooltip("StFuGAME: starting…")
            .build()
            .expect("creating the notification area icon");
        Self { tray, start, stop, log, dash, eod, show, quit }
    }

    pub fn set_state(&self, state: State) {
        let tip = match state {
            State::Running => "StFuGAME: at least one character running",
            State::Stopped => "StFuGAME: all characters stopped",
            State::Failed => "StFuGAME: all characters ended with an error (see log)",
        };
        let _ = self.tray.set_icon(Some(icon(state)));
        let _ = self.tray.set_tooltip(Some(tip));
    }
}
