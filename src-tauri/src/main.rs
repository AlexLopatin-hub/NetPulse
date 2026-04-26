// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    thread,
    time::{Duration, Instant},
};
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Emitter,
};

#[derive(Clone, serde::Serialize)]
struct PingData {
    latency: u32,
    jitter: u32,
    loss: f32,
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let quit_item = MenuItemBuilder::with_id("quit", "Выход").build(app)?;

            let menu = MenuBuilder::new(app).item(&quit_item).build()?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("NetPulse")
                .menu(&menu)
                .on_menu_event(|app, event| {
                    if event.id() == "quit" {
                        app.exit(0);
                    }
                })
                .build(app)?;

            let app_handle = app.handle().clone();

            thread::spawn(move || {
                let target: SocketAddr = "1.1.1.1:80".parse().unwrap();

                let mut history: Vec<bool> = Vec::new();
                let mut last_latency: Option<u32> = None;

                loop {
                    let timeout = Duration::from_secs(1);

                    let mut current_latency = 0;
                    let mut is_success = false;
                    let mut current_jitter = 0;

                    if let Ok(mut stream) = TcpStream::connect_timeout(&target, timeout) {
                        let _ = stream.set_read_timeout(Some(timeout));
                        let _ = stream.set_write_timeout(Some(timeout));

                        let start_time = Instant::now();

                        let request =
                            b"HEAD / HTTP/1.1\r\nHost: 1.1.1.1\r\nConnection: close\r\n\r\n";

                        if stream.write_all(request).is_ok() {
                            let mut buffer = [0; 1];

                            if stream.read_exact(&mut buffer).is_ok() {
                                current_latency = start_time.elapsed().as_millis() as u32;
                                is_success = true;

                                if let Some(last) = last_latency {
                                    current_jitter = current_latency.abs_diff(last);
                                }
                                last_latency = Some(current_latency);
                            }
                        }
                        let _ = stream.shutdown(std::net::Shutdown::Both);
                    }

                    if !is_success {
                        last_latency = None;
                    }

                    history.push(is_success);
                    if history.len() > 50 {
                        history.remove(0);
                    }

                    let lost_count = history.iter().filter(|&&x| !x).count();
                    let loss_percent = (lost_count as f32 / history.len() as f32) * 100.0;

                    let _ = app_handle.emit(
                        "ping_update",
                        PingData {
                            latency: current_latency,
                            jitter: current_jitter,
                            loss: loss_percent,
                        },
                    );

                    thread::sleep(Duration::from_secs(1));
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Ошибка при запуске приложения Tauri");
}
