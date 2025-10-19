// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod network;
mod server;
mod file_handler;

use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{Manager, State, Window};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingConnection {
    pub id: String,
    pub device_name: String,
    pub device_ip: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingFile {
    pub id: String,
    pub file_name: String,
    pub file_size: u64,
    pub sender_name: String,
    pub sender_ip: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

// Application state
pub struct AppState {
    pub server: Arc<Mutex<Option<server::FileServer>>>,
    pub devices: Arc<Mutex<Vec<network::Device>>>,
    pub pending_connections: Arc<Mutex<Vec<IncomingConnection>>>,
    pub pending_files: Arc<Mutex<Vec<IncomingFile>>>,
    pub window: Arc<Mutex<Option<Window>>>,
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

#[tauri::command]
async fn get_pending_connections(state: State<'_, AppState>) -> Result<Vec<IncomingConnection>, String> {
    let connections_guard = state.pending_connections.lock().await;
    Ok(connections_guard.clone())
}

#[tauri::command]
async fn get_pending_files(state: State<'_, AppState>) -> Result<Vec<IncomingFile>, String> {
    let files_guard = state.pending_files.lock().await;
    Ok(files_guard.clone())
}

#[tauri::command]
async fn approve_connection(connection_id: String, approved: bool, state: State<'_, AppState>) -> Result<String, String> {
    let mut connections_guard = state.pending_connections.lock().await;
    
    if let Some(index) = connections_guard.iter().position(|c| c.id == connection_id) {
        let connection = connections_guard.remove(index);
        
        if approved {
            // Add to approved devices
            let mut devices_guard = state.devices.lock().await;
            devices_guard.push(network::Device::new(
                connection.device_name,
                connection.device_ip,
                8080
            ));
            Ok("Connection approved".to_string())
        } else {
            Ok("Connection rejected".to_string())
        }
    } else {
        Err("Connection not found".to_string())
    }
}

#[tauri::command]
async fn approve_file_transfer(file_id: String, approved: bool, save_path: Option<String>, state: State<'_, AppState>) -> Result<String, String> {
    let mut files_guard = state.pending_files.lock().await;
    
    if let Some(index) = files_guard.iter().position(|f| f.id == file_id) {
        let file = files_guard.remove(index);
        
        if approved {
            if let Some(path) = save_path {
                // Start file download to specified path
                tokio::spawn(async move {
                    if let Err(e) = file_handler::download_file(&file.sender_ip, &file_id, &path).await {
                        eprintln!("Failed to download file: {}", e);
                    }
                });
                Ok("File transfer approved".to_string())
            } else {
                Err("Save path required for file approval".to_string())
            }
        } else {
            Ok("File transfer rejected".to_string())
        }
    } else {
        Err("File not found".to_string())
    }
}

#[tauri::command]
async fn select_save_directory() -> Result<Option<String>, String> {
    use tauri::api::dialog;
    
    match dialog::blocking::FileDialogBuilder::new()
        .set_title("Select Save Directory")
        .pick_folder() {
        Some(path) => Ok(Some(path.to_string_lossy().to_string())),
        None => Ok(None),
    }
}

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            server: Arc::new(Mutex::new(None)),
            devices: Arc::new(Mutex::new(Vec::new())),
            pending_connections: Arc::new(Mutex::new(Vec::new())),
            pending_files: Arc::new(Mutex::new(Vec::new())),
            window: Arc::new(Mutex::new(None)),
        })
        .invoke_handler(tauri::generate_handler![
            start_server,
            stop_server,
            discover_devices,
            get_devices,
            send_file,
            get_local_ip,
            get_pending_connections,
            get_pending_files,
            approve_connection,
            approve_file_transfer,
            select_save_directory
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
