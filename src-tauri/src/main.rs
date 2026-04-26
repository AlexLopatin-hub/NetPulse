// Prevents additional console window on Windows in release, DO NOT REMOVE!!
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
use tokio::net::TcpStream;
use tokio::time::{timeout, Instant};
use tokio_rustls::TlsConnector;

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
            jitter: true,
            loss: true,
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
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.emit("settings_updated", settings);
    }
}

#[tauri::command]
fn resize_widget(app: tauri::AppHandle, width: u32, height: u32) {
    // Ваша реализация resize_widget
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
    let margin_phys = (MARGIN_LOGICAL as f64 * scale).round() as i32;
    let x = screen.width as i32 - win_w as i32 - margin_phys;
    let y = margin_phys;
    let _ = win.set_size(tauri::PhysicalSize::new(win_w, win_h));
    let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
}

fn open_settings_window(app: &tauri::AppHandle) {
    // Ваша реализация open_settings_window
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

/// Асинхронная проверка доступности через TLS-рукопожатие
async fn probe_host(
    entry: &HostEntry,
    resolver: &TokioAsyncResolver,
    tls_connector: &TlsConnector,
    last_latency: &mut Option<u32>,
) -> (u32, u32, bool) {
    let timeout_duration = Duration::from_secs(2);

    // Независимый DNS-резолвинг через DoT
    let Ok(lookup_res) = timeout(timeout_duration, resolver.lookup_ip(&entry.address)).await else {
        *last_latency = None;
        return (0, 0, false);
    };

    let Some(ip) = lookup_res.ok().and_then(|r| r.iter().next()) else {
        *last_latency = None;
        return (0, 0, false);
    };

    let addr = std::net::SocketAddr::new(ip, entry.port);
    let start = Instant::now();

    // Установление TCP (ТСПУ этот этап игнорирует)
    let Ok(tcp_res) = timeout(timeout_duration, TcpStream::connect(addr)).await else {
        *last_latency = None;
        return (0, 0, false);
    };

    let Ok(stream) = tcp_res else {
        *last_latency = None;
        return (0, 0, false);
    };

    // Отключаем алгоритм Нейгла для моментальной отправки пакетов
    let _ = stream.set_nodelay(true);

    // Отправка TLS ClientHello с полем SNI (триггер блокировки)
    let Ok(server_name) = ServerName::try_from(entry.address.as_str()) else {
        return (0, 0, false);
    };

    let tls_handshake = tls_connector.connect(server_name.to_owned(), stream);

    // Если ТСПУ блокирует ресурс, здесь мы получим Timeout (Silent Drop)
    // или ErrorKind::ConnectionReset (RST инъекция)
    match timeout(timeout_duration, tls_handshake).await {
        Ok(Ok(_tls_stream)) => {
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
        .manage(shared.clone())
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            resize_widget,
        ])
        .setup(move |app| {
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

                // Подготовка сертификатов для Rustls
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
