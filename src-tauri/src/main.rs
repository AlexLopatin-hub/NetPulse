#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use hickory_resolver::{config::*, TokioAsyncResolver};
use rustls::pki_types::ServerName;
use rustls::RootCertStore;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Emitter, Manager,
};
use tauri_plugin_store::StoreExt;
use tokio::net::TcpStream;
use tokio::time::{timeout, Instant};
use tokio_rustls::TlsConnector;

const STORE_FILE: &str = "netpulse.json";

#[derive(Clone, serde::Serialize)]
struct PingData {
    host: String,
    latency: u32,
    jitter: u32,
    loss: f32,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct HostEntry {
    label: String,
    address: String,
    port: u16,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct ColumnConfig {
    latency: bool,
    jitter: bool,
    loss: bool,
}

impl Default for ColumnConfig {
    fn default() -> Self {
        Self {
            latency: true,
            jitter: false,
            loss: false,
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Settings {
    hosts: Vec<HostEntry>,
    columns: ColumnConfig,
}

type SharedSettings = Arc<Mutex<Settings>>;

fn default_settings() -> Settings {
    Settings {
        hosts: vec![
            HostEntry {
                label: "YouTube".into(),
                address: "youtube.com".into(),
                port: 443,
            },
            HostEntry {
                label: "Discord".into(),
                address: "discord.com".into(),
                port: 443,
            },
            HostEntry {
                label: "Telegram".into(),
                address: "telegram.org".into(),
                port: 443,
            },
        ],
        columns: ColumnConfig::default(),
    }
}

#[tauri::command]
fn get_settings(shared: tauri::State<SharedSettings>) -> Settings {
    shared.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(shared: tauri::State<SharedSettings>, app: tauri::AppHandle, settings: Settings) {
    {
        *shared.lock().unwrap() = settings.clone();
    }
    if let Ok(store) = app.store(STORE_FILE) {
        let _ = store.set("settings", serde_json::to_value(&settings).unwrap());
        let _ = store.save();
    }
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.emit("settings_updated", settings);
    }
}

#[tauri::command]
fn resize_widget(app: tauri::AppHandle, width: u32, height: u32) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let Ok(Some(monitor)) = win.current_monitor() else {
        return;
    };

    let scale = monitor.scale_factor();
    let screen = monitor.size();

    const MARGIN_LOGICAL: u32 = 14;
    let win_w = ((width + MARGIN_LOGICAL * 2) as f64 * scale).round() as u32;
    let win_h = ((height + MARGIN_LOGICAL * 2) as f64 * scale).round() as u32;

    let saved = app.store(STORE_FILE).ok().and_then(|store| {
        let x = store.get("win_x")?.as_i64()? as i32;
        let y = store.get("win_y")?.as_i64()? as i32;
        Some((x, y))
    });

    let (x, y) = if let Some((saved_x, saved_y)) = saved {
        let right_gap = screen.width as i32 - saved_x - win_w as i32;
        if right_gap.abs() < (300.0 * scale) as i32 {
            (screen.width as i32 - win_w as i32 - right_gap, saved_y)
        } else {
            default_pos(screen.width, win_w, scale)
        }
    } else {
        default_pos(screen.width, win_w, scale)
    };

    let _ = win.set_size(tauri::PhysicalSize::new(win_w, win_h));
    let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
}

fn default_pos(screen_w: u32, win_w: u32, scale: f64) -> (i32, i32) {
    let margin = (14.0 * scale).round() as i32;
    let first_run_offset = (100.0 * scale).round() as i32;
    (
        screen_w as i32 - win_w as i32 - margin,
        margin + first_run_offset,
    )
}

#[tauri::command]
fn save_position(app: tauri::AppHandle) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let Ok(pos) = win.outer_position() else {
        return;
    };
    if let Ok(store) = app.store(STORE_FILE) {
        let _ = store.set("win_x", serde_json::json!(pos.x));
        let _ = store.set("win_y", serde_json::json!(pos.y));
        let _ = store.save();
    }
}

fn open_settings_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }
    let _ = tauri::WebviewWindowBuilder::new(
        app,
        "settings",
        tauri::WebviewUrl::App("/settings".into()),
    )
    .title("Настройки — NetPulse")
    .inner_size(480.0, 460.0)
    .build();
}

async fn probe_host(
    entry: &HostEntry,
    resolver: &TokioAsyncResolver,
    tls_connector: &TlsConnector,
    last_latency: &mut Option<u32>,
) -> (u32, u32, bool) {
    let tout = Duration::from_secs(2);

    let Ok(lookup) = timeout(tout, resolver.lookup_ip(&entry.address)).await else {
        *last_latency = None;
        return (0, 0, false);
    };
    let Some(ip) = lookup.ok().and_then(|r| r.iter().next()) else {
        *last_latency = None;
        return (0, 0, false);
    };

    let addr = std::net::SocketAddr::new(ip, entry.port);
    let start = Instant::now();

    let Ok(tcp) = timeout(tout, TcpStream::connect(addr)).await else {
        *last_latency = None;
        return (0, 0, false);
    };
    let Ok(stream) = tcp else {
        *last_latency = None;
        return (0, 0, false);
    };
    let _ = stream.set_nodelay(true);

    let Ok(server_name) = ServerName::try_from(entry.address.as_str()) else {
        return (0, 0, false);
    };

    match timeout(tout, tls_connector.connect(server_name.to_owned(), stream)).await {
        Ok(Ok(_)) => {
            let latency = start.elapsed().as_millis() as u32;
            let jitter = last_latency.map(|l| latency.abs_diff(l)).unwrap_or(0);
            *last_latency = Some(latency);
            (latency, jitter, true)
        }
        _ => {
            *last_latency = None;
            (0, 0, false)
        }
    }
}

fn main() {
    let shared: SharedSettings = Arc::new(Mutex::new(default_settings()));

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(shared.clone())
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            resize_widget,
            save_position,
        ])
        .setup(move |app| {
            let store = app.store(STORE_FILE)?;
            if let Some(val) = store.get("settings") {
                if let Ok(s) = serde_json::from_value::<Settings>(val) {
                    *shared.lock().unwrap() = s;
                }
            }

            if let Some(win) = app.get_webview_window("main") {
                let saved = {
                    let x = store
                        .get("win_x")
                        .and_then(|v| v.as_i64())
                        .map(|v| v as i32);
                    let y = store
                        .get("win_y")
                        .and_then(|v| v.as_i64())
                        .map(|v| v as i32);
                    x.zip(y)
                };
                if let Ok(Some(monitor)) = win.current_monitor() {
                    let scale = monitor.scale_factor();
                    let screen = monitor.size();
                    let (x, y) = saved.unwrap_or_else(|| {
                        let init_w = (260.0 * scale).round() as u32;
                        default_pos(screen.width, init_w, scale)
                    });
                    let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
                }
            }

            let quit_item = MenuItemBuilder::with_id("quit", "Выход").build(app)?;
            let settings_item = MenuItemBuilder::with_id("settings", "Настройки").build(app)?;
            let menu = MenuBuilder::new(app)
                .item(&settings_item)
                .item(&quit_item)
                .build()?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("NetPulse")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "quit" => app.exit(0),
                    "settings" => open_settings_window(app),
                    _ => {}
                })
                .build(app)?;

            let app_handle = app.handle().clone();
            let shared_ping = shared.clone();

            tauri::async_runtime::spawn(async move {
                let mut opts = ResolverOpts::default();
                opts.timeout = Duration::from_secs(2);
                opts.attempts = 2;
                let resolver = TokioAsyncResolver::tokio(ResolverConfig::default(), opts);

                let mut root_store = RootCertStore::empty();
                root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
                let config = rustls::ClientConfig::builder()
                    .with_root_certificates(root_store)
                    .with_no_client_auth();
                let tls_connector = TlsConnector::from(Arc::new(config));

                let mut host_state: std::collections::HashMap<String, (Vec<bool>, Option<u32>)> =
                    std::collections::HashMap::new();

                loop {
                    let hosts_snapshot = shared_ping.lock().unwrap().hosts.clone();
                    host_state.retain(|k, _| hosts_snapshot.iter().any(|h| &h.label == k));

                    for entry in &hosts_snapshot {
                        let (history, last_latency) = host_state
                            .entry(entry.label.clone())
                            .or_insert_with(|| (Vec::new(), None));

                        let (latency, jitter, success) =
                            probe_host(entry, &resolver, &tls_connector, last_latency).await;

                        history.push(success);
                        if history.len() > 10 {
                            history.remove(0);
                        }

                        let lost = history.iter().filter(|&&x| !x).count();
                        let loss = (lost as f32 / history.len() as f32) * 100.0;

                        let _ = app_handle.emit(
                            "ping_update",
                            PingData {
                                host: entry.label.clone(),
                                latency,
                                jitter,
                                loss,
                            },
                        );
                    }

                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Ошибка при запуске приложения Tauri");
}
