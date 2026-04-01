// src-tauri/src/lib.rs
mod blocker;
mod tray;

use std::{
    borrow::Cow,
    fs,
    io,
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    path::PathBuf,
    time::Duration,
};
use tauri::{
    command, AppHandle, Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

const CONTENT_JS: &str = include_str!("content.js");
const OFFLINE_HTML: &str = include_str!("offline.html");
const SNAPSHOT_FILE: &str = "offline_snapshot.json";
const WHATSAPP_URL: &str = "https://web.whatsapp.com";

#[inline]
fn set_badge<R: Runtime>(window: &WebviewWindow<R>, count: Option<i64>) {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    let _ = window.set_badge_count(count);
    let _ = (window, count);
}

const FAST_POLL: Duration = Duration::from_secs(3);
const SLOW_POLL: Duration = Duration::from_secs(15);

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

fn start_poller<R: Runtime + 'static>(app: tauri::AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        let mut last: i64 = -1;

        loop {
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
                continue;
            }
            last = count;

            set_badge(&window, (count > 0).then_some(count));

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

fn app_data_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))
}

fn snapshot_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join(SNAPSHOT_FILE))
}

fn set_persistent_webview_data_dir<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let dir = app_data_dir(app)?.join("webview");
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create webview dir: {e}"))?;
    std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", dir);
    Ok(())
}

fn is_network_available() -> bool {
    let addr = ("web.whatsapp.com", 443)
        .to_socket_addrs()
        .ok()
        .and_then(|mut addrs| addrs.next())
        .unwrap_or_else(|| SocketAddr::from(([1, 1, 1, 1], 443)));

    TcpStream::connect_timeout(&addr, Duration::from_secs(2)).is_ok()
}

#[command]
fn save_snapshot(app: AppHandle, snapshot: String) -> Result<(), String> {
    let path = snapshot_path(&app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("failed to create data dir: {e}"))?;
    }
    fs::write(path, snapshot).map_err(|e| format!("failed to write snapshot: {e}"))
}

#[command]
fn get_snapshot(app: AppHandle) -> Result<String, String> {
    let path = snapshot_path(&app)?;
    fs::read_to_string(path).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => {
            "No offline data available yet. Connect once to WhatsApp Web to build a cache snapshot."
                .to_string()
        }
        _ => format!("failed to read snapshot: {e}"),
    })
}

#[command]
fn open_external(url: String) -> Result<(), String> {
    tauri::webbrowser::open(&url)
        .map(|_| ())
        .map_err(|e| format!("failed to open URL: {e}"))
}

pub fn run() {
    tauri::async_runtime::spawn_blocking(blocker::init);

    tauri::Builder::default()
        .setup(|app| {
            let _ = set_persistent_webview_data_dir(app.handle());

            let url = if is_network_available() {
                WebviewUrl::External(WHATSAPP_URL.parse().unwrap())
            } else {
                let encoded = urlencoding::encode(OFFLINE_HTML).to_string();
                WebviewUrl::External(format!("data:text/html;charset=utf-8,{encoded}").parse().unwrap())
            };

            WebviewWindowBuilder::new(app, "main", url)
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
        .invoke_handler(tauri::generate_handler![save_snapshot, get_snapshot, open_external])
        .run(tauri::generate_context!())
        .expect("error while running sup");
}
