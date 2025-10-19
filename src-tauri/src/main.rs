// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod network;
mod server;
mod file_handler;

use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{Manager, State};

// Application state
pub struct AppState {
    pub server: Arc<Mutex<Option<server::FileServer>>>,
    pub devices: Arc<Mutex<Vec<network::Device>>>,
}

#[tauri::command]
async fn start_server(state: State<'_, AppState>) -> Result<String, String> {
    let mut server_guard = state.server.lock().await;
    
    if server_guard.is_some() {
        return Ok("Server already running".to_string());
    }

    match server::FileServer::new().await {
        Ok(server) => {
            let server_arc = Arc::new(server);
            let server_clone = server_arc.clone();
            
            // Start the server in a background task
            tokio::spawn(async move {
                if let Err(e) = server_clone.start().await {
                    eprintln!("Server error: {}", e);
                }
            });

            *server_guard = Some(server_arc);
            Ok("Server started successfully".to_string())
        }
        Err(e) => Err(format!("Failed to start server: {}", e)),
    }
}

#[tauri::command]
async fn stop_server(state: State<'_, AppState>) -> Result<String, String> {
    let mut server_guard = state.server.lock().await;
    *server_guard = None;
    Ok("Server stopped".to_string())
}

#[tauri::command]
async fn discover_devices(state: State<'_, AppState>) -> Result<Vec<network::Device>, String> {
    let devices = network::discover_devices().await?;
    let mut devices_guard = state.devices.lock().await;
    *devices_guard = devices.clone();
    Ok(devices)
}

#[tauri::command]
async fn get_devices(state: State<'_, AppState>) -> Result<Vec<network::Device>, String> {
    let devices_guard = state.devices.lock().await;
    Ok(devices_guard.clone())
}

#[tauri::command]
async fn send_file(device_ip: String, file_path: String) -> Result<String, String> {
    file_handler::send_file_to_device(&device_ip, &file_path).await
}

#[tauri::command]
async fn get_local_ip() -> Result<String, String> {
    match local_ip_address::local_ip() {
        Ok(ip) => Ok(ip.to_string()),
        Err(e) => Err(format!("Failed to get local IP: {}", e)),
    }
}

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            server: Arc::new(Mutex::new(None)),
            devices: Arc::new(Mutex::new(Vec::new())),
        })
        .invoke_handler(tauri::generate_handler![
            start_server,
            stop_server,
            discover_devices,
            get_devices,
            send_file,
            get_local_ip
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
