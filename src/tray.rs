//! Ikona v oznamovací oblasti (vedle hodin): spuštění a zastavení bota, otevření logu, ukončení.
//! Zelená = bot běží, šedá = zastavený, červená = skončil chybou (viz log).
//!
//! Jediné místo s `unsafe`: volání Windows API (obdoba P/Invoke v C#) pro smyčku zpráv,
//! kterou ikona potřebuje, a pro hlídání, že běží jen jedna instance.

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

/// Běží už jiná instance bota? (pojmenovaný mutex Windows, drží se do konce procesu)
pub fn already_running() -> bool {
    let name = wide("Local\\StFuGAME_bot");
    // SAFETY: platný nulou ukončený UTF-16 řetězec; handle záměrně nezavíráme (drží mutex do konce procesu)
    unsafe {
        CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        GetLastError() == ERROR_ALREADY_EXISTS
    }
}

/// Informační okno (např. „bot už běží“).
pub fn message_box(text: &str) {
    let (text, caption) = (wide(text), wide("StFuGAME"));
    // SAFETY: platné nulou ukončené UTF-16 řetězce, bez rodičovského okna
    unsafe {
        MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), MB_ICONINFORMATION);
    }
}

/// Kulatá ikona 32×32 v dané barvě.
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
    Icon::from_rgba(rgba, size, size).expect("ikona z platných RGBA dat")
}

fn set_state(tray: &TrayIcon, start: &MenuItem, stop: &MenuItem, state: State) {
    let tip = match state {
        State::Running => "StFuGAME: bot běží",
        State::Stopped => "StFuGAME: bot zastavený",
        State::Failed => "StFuGAME: bot skončil chybou (viz log)",
    };
    let _ = tray.set_icon(Some(icon(state)));
    let _ = tray.set_tooltip(Some(tip));
    start.set_enabled(state != State::Running);
    stop.set_enabled(state == State::Running);
}

/// Spustí ikonu a hned i bota. Blokuje, dokud uživatel nezvolí „Ukončit“.
pub fn run<F, Fut>(rt: &Runtime, bot: F)
where
    F: Fn() -> Fut,
    Fut: Future<Output = ExitCode> + Send + 'static,
{
    let menu = Menu::new();
    let start = MenuItem::new("Spustit bota", false, None);
    let stop = MenuItem::new("Zastavit bota", true, None);
    let log = MenuItem::new("Otevřít log", true, None);
    let quit = MenuItem::new("Ukončit", true, None);
    let _ = menu.append_items(&[&start, &stop, &PredefinedMenuItem::separator(), &log, &PredefinedMenuItem::separator(), &quit]);

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon(State::Running))
        .with_tooltip("StFuGAME: bot běží")
        .build()
        .expect("vytvoření ikony v oznamovací oblasti");

    let mut handle: Option<JoinHandle<ExitCode>> = Some(rt.spawn(bot()));
    report!("[ovládání] Bot spuštěn");
    let mut state = State::Running;
    set_state(&tray, &start, &stop, state);

    loop {
        // Smyčka zpráv Windows (ikona bez ní nereaguje)
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        // SAFETY: standardní smyčka zpráv nad platnou strukturou MSG
        unsafe {
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == start.id() && state != State::Running {
                handle = Some(rt.spawn(bot()));
                report!("[ovládání] Bot spuštěn");
                state = State::Running;
                set_state(&tray, &start, &stop, state);
            } else if event.id == stop.id() && state == State::Running {
                if let Some(h) = handle.take() {
                    h.abort();
                }
                report!("[ovládání] Bot zastaven");
                state = State::Stopped;
                set_state(&tray, &start, &stop, state);
            } else if event.id == log.id() {
                let _ = std::process::Command::new("explorer").arg("logs\\prubeh.log").spawn();
            } else if event.id == quit.id() {
                if let Some(h) = handle.take() {
                    h.abort();
                }
                report!("[ovládání] Ukončeno");
                return;
            }
        }

        // Bot skončil sám (chyba přihlášení apod.)
        if state == State::Running && handle.as_ref().is_some_and(JoinHandle::is_finished) {
            handle = None;
            report!("[ovládání] Bot skončil sám, podrobnosti v logu");
            state = State::Failed;
            set_state(&tray, &start, &stop, state);
        }

        std::thread::sleep(Duration::from_millis(50));
    }
}
