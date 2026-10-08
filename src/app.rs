//! The app window (user 2026-10-08): one desktop window with character tiles (individual start/stop) and a
//! "Charts" tab (the dashboard). Built with `tao` (window + the single Win32 message loop, also pumping the
//! tray icon's events) and `wry` (an OS WebView2 control showing `app.html`, which embeds `dashboard.html`
//! in an iframe). Closing the window only hides it; the tray icon's "Exit" ends the process.
//!
//! Served over the `app://` custom protocol (`app://localhost/…`), NOT `file://` (bug found 2026-10-08:
//! Chromium/WebView2 treats every `file://` page as its own unique, untrusted origin, which silently breaks
//! both wry's `window.ipc` injection and the dashboard iframe – console: "'file:' URLs are treated as unique
//! security origins", no exception, nothing in the log, just a native message that quietly goes nowhere).

use std::{borrow::Cow, path::Path};

use tao::{
    dpi::LogicalSize,
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use tokio::runtime::Runtime;
use tray_icon::menu::MenuEvent;
use wry::{WebViewBuilder, http};

use crate::{control, tray};

/// How often the window gets a fresh status (tray icon's "is anything running" + the app's tiles).
const TICK: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Debug)]
enum AppEvent {
    Tick,
}

/// Serves files from `roster/` over the `app://` custom protocol (see module doc for why not `file://`).
fn serve_asset(_id: wry::WebViewId, request: http::Request<Vec<u8>>) -> http::Response<Cow<'static, [u8]>> {
    let path = request.uri().path().trim_start_matches('/');
    let path = if path.is_empty() { "app.html" } else { path };
    let content_type = if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".js") {
        "text/javascript; charset=utf-8"
    } else {
        "application/octet-stream"
    };
    let empty = || http::Response::new(Cow::Borrowed(&[] as &[u8]));
    match std::fs::read(Path::new("roster").join(path)) {
        Ok(bytes) => http::Response::builder()
            .header("Content-Type", content_type)
            .body(Cow::Owned(bytes))
            .unwrap_or_else(|_| empty()),
        Err(e) => {
            report!("[control] App window: asset {path} not found: {e}");
            http::Response::builder().status(404).body(Cow::Borrowed(&[] as &[u8])).unwrap_or_else(|_| empty())
        }
    }
}

/// Builds the window + webview and runs the one message loop for the whole app (icon + window). Never returns.
pub fn run(rt: Runtime, accounts: Vec<crate::Credentials>) -> ! {
    control::register(&accounts);

    let _ = std::fs::create_dir_all("roster");
    let _ = std::fs::write("roster/app.html", include_str!("app.html"));

    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(TICK);
            if proxy.send_event(AppEvent::Tick).is_err() {
                return; // the event loop ended (app exited)
            }
        }
    });

    let window = WindowBuilder::new()
        .with_title("StFuGAME")
        .with_inner_size(LogicalSize::new(1080.0, 760.0))
        .with_visible(true)
        .build(&event_loop)
        .expect("creating the app window");

    let rt_handle = rt.handle().clone();
    let webview = WebViewBuilder::new()
        .with_custom_protocol("app".to_string(), serve_asset)
        .with_url("app://localhost/app.html")
        .with_ipc_handler(move |req| handle_ipc(&rt_handle, req.body()))
        // Right click → Inspect (or F12) to see the console if the window ever misbehaves again.
        .with_devtools(true)
        .build(&window)
        .expect("creating the embedded web view");

    let tray = tray::Tray::build();
    tray.set_state(tray::State::Stopped);

    // Only characters the user previously, explicitly switched on log in right away (staggered); everyone else
    // stays off until switched on – including on a brand new install, opening the app must never start anything
    // by itself (user 2026-10-08). STFU_NO_LOGIN=1 skips this entirely (for trying out the window, no network).
    if std::env::var("STFU_NO_LOGIN").is_ok_and(|v| v == "1") {
        report!("[control] STFU_NO_LOGIN=1: not logging any character in");
    } else {
        let auto: Vec<&str> =
            accounts.iter().filter(|c| control::should_autostart(&c.character)).map(|c| c.character.as_str()).collect();
        for (i, name) in auto.iter().enumerate() {
            let delay = if i == 0 { 0 } else { i as u64 * 20 + fastrand::u64(0..20) };
            control::start(rt.handle(), name, delay);
        }
        report!(
            "[control] Bot started ({} of {} character(s) autostart)",
            auto.len(),
            control::names().len()
        );
    }

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::NewEvents(StartCause::Init) => {}
            Event::UserEvent(AppEvent::Tick) => {
                let status = control::status_json();
                let _ = webview.evaluate_script(&format!("window.onStatus && window.onStatus({status})"));
                tray.set_state(control::overall());
            }
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                // Keep running in the background; the tray icon's "Exit" is the real quit.
                window.set_visible(false);
            }
            _ => {}
        }

        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            if ev.id == tray.show.id() {
                window.set_visible(true);
                window.set_focus();
            } else if ev.id == tray.start.id() {
                control::start_all(rt.handle());
                report!("[control] Start all requested from the tray icon");
            } else if ev.id == tray.stop.id() {
                control::stop_all();
                report!("[control] Stop all requested from the tray icon");
            } else if ev.id == tray.eod.id() {
                report!("[control] End of day requested (preview)");
                crate::ctx::request_end_of_day();
            } else if ev.id == tray.dash.id() {
                open_dashboard();
            } else if ev.id == tray.log.id() {
                let _ = std::process::Command::new("explorer").arg("logs\\progress.log").spawn();
            } else if ev.id == tray.quit.id() {
                report!("[control] Exited");
                std::process::exit(0);
            }
        }
    })
}

fn open_dashboard() {
    let _ = std::process::Command::new("explorer").arg("roster\\dashboard.html").spawn();
}

/// One message from `app.html`'s `window.ipc.postMessage`.
fn handle_ipc(rt: &tokio::runtime::Handle, body: &str) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(body) else {
        report!("[control] Could not parse a command from the app window: {body}");
        return;
    };
    let cmd = v["cmd"].as_str().unwrap_or_default();
    let name = v["name"].as_str().unwrap_or_default();
    report!("[control] App window: {cmd} {name}");
    match cmd {
        "start" if !name.is_empty() => control::start(rt, name, 0),
        "stop" if !name.is_empty() => control::stop(name),
        "start_all" => control::start_all(rt),
        "stop_all" => control::stop_all(),
        "eod" => crate::ctx::request_end_of_day(),
        "open_browser" => open_dashboard(),
        other => report!("[control] Unknown command from the app window: {other:?}"),
    }
}
