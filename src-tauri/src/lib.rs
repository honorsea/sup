// src-tauri/src/lib.rs
//
// sup — Tauri WhatsApp Web client
//
// Architecture:
//   • Window loads web.whatsapp.com directly via WebviewUrl::External.
//   • content.js (injected at startup) observes document.title via
//     MutationObserver and writes the unread count to window.__sup_unread__.
//   • A tokio green task polls window.title() from the Rust side every
//     3 s (visible) or 15 s (hidden/minimized) and updates the taskbar
//     badge and tray tooltip when the count changes.
//   • No Tauri IPC commands are exposed to the page (withGlobalTauri: false).
//   • Tracker blocking: adblock engine initialized in the background thread
//     pool, ready for future WebView2 network interception support.

mod blocker;
mod tray;

use std::borrow::Cow;
use std::time::Duration;
use tauri::{Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

const CONTENT_JS: &str = include_str!("inject/content.js");

// ─── Badge ────────────────────────────────────────────────────────────────────

/// Update the taskbar/dock badge for the given window.
/// Compiled away on platforms that support neither badge API.
#[inline]
fn set_badge<R: Runtime>(window: &WebviewWindow<R>, count: Option<i64>) {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    let _ = window.set_badge_count(count);
    let _ = (window, count);
}

// ─── Unread Polling ───────────────────────────────────────────────────────────

const FAST_POLL: Duration = Duration::from_secs(3); // window visible
const SLOW_POLL: Duration = Duration::from_secs(15); // window hidden / minimized

/// Parse "(42) WhatsApp" → 42, anything else → 0.
/// Fast-rejects on the first character to avoid any parsing in the common
/// "no unread" case where the title is simply "WhatsApp".
#[inline]
fn parse_unread(title: &str) -> i64 {
    if !title.starts_with('(') {
        return 0;
    }
    title
        .strip_prefix('(')
        .and_then(|s| s.split_once(')'))
        .map(|(n, _)| n)
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

/// Spawns a tokio green task that polls the window title and updates the
/// taskbar badge + tray tooltip whenever the unread count changes.
///
/// Uses adaptive sleep: 3 s when the window is visible (badge update matters
/// immediately), 15 s when hidden/minimized (saves CPU wakeups).
fn start_poller<R: Runtime + 'static>(app: tauri::AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        let mut last: i64 = -1; // -1 forces an update on the first iteration

        loop {
            // Choose sleep duration based on current window visibility.
            let interval = app
                .get_webview_window("main")
                .map(|w| {
                    let visible = w.is_visible().unwrap_or(true);
                    let minimized = w.is_minimized().unwrap_or(false);
                    if visible && !minimized {
                        FAST_POLL
                    } else {
                        SLOW_POLL
                    }
                })
                .unwrap_or(SLOW_POLL);

            tokio::time::sleep(interval).await;

            let window = match app.get_webview_window("main") {
                Some(w) => w,
                None => continue,
            };

            let count = match window.title() {
                Ok(t) => parse_unread(&t),
                Err(_) => continue,
            };

            if count == last {
                continue; // nothing changed — skip all system calls
            }
            last = count;

            // Taskbar badge
            set_badge(&window, (count > 0).then_some(count));

            // Tray tooltip — Cow avoids a heap allocation in the common (0) case.
            let tip: Cow<'static, str> = if count > 0 {
                Cow::Owned(format!("sup — {} unread", count))
            } else {
                Cow::Borrowed("sup — WhatsApp")
            };
            if let Some(tray) = app.tray_by_id("main_tray") {
                let _ = tray.set_tooltip(Some(tip.as_ref()));
            }
        }
    });
}

// ─── Entry Point ──────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Warm up the adblock engine in the background — hidden behind startup.
    tauri::async_runtime::spawn_blocking(blocker::init);

    tauri::Builder::default()
        .setup(|app| {
            WebviewWindowBuilder::new(
                app,
                "main",
                WebviewUrl::External("https://web.whatsapp.com".parse().unwrap()),
            )
            .title("sup")
            .inner_size(1280.0, 840.0)
            .min_inner_size(800.0, 600.0)
            .center()
            .resizable(true)
            .initialization_script(CONTENT_JS)
            .build()?;

            tray::setup_tray(app.handle())?;
            start_poller(app.handle().clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running sup");
}
