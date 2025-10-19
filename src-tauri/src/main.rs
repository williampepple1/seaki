// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{State};
use serde::{Deserialize, Serialize};

mod server;
mod network;
mod file_handler;

use server::FileServer;
use network::NetworkDiscovery;
use file_handler::FileHandler;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub last_seen: chrono::DateTime<chrono::Utc>,
}

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
    pub pending_connections: Arc<Mutex<Vec<IncomingConnection>>>,
    pub pending_files: Arc<Mutex<Vec<IncomingFile>>>,
    pub server: Arc<Mutex<Option<Arc<FileServer>>>>,
    pub network_discovery: Arc<Mutex<NetworkDiscovery>>,
    pub file_handler: Arc<FileHandler>,
    pub is_server_running: Arc<Mutex<bool>>,
}

#[tauri::command]
async fn start_server(state: State<'_, AppState>) -> Result<String, String> {
    let mut server_guard = state.server.lock().await;
    let mut is_running_guard = state.is_server_running.lock().await;
    
    if *is_running_guard {
        return Ok("Server is already running".to_string());
    }
    
    // Use a specific weird port that's unlikely to be used by other applications
    let port = 54321;
    let file_server = FileServer::new(port);
    
    // Start the server in a separate task
    let server_arc = Arc::new(file_server);
    let server_clone = server_arc.clone();
    
    tokio::spawn(async move {
        if let Err(e) = server_clone.start().await {
            log::error!("Server error: {}", e);
        }
    });
    
    *server_guard = Some(server_arc);
    *is_running_guard = true;
    
    // Start network discovery
    let mut discovery_guard = state.network_discovery.lock().await;
    if let Err(e) = discovery_guard.start_discovery().await {
        log::error!("Failed to start discovery: {}", e);
    }
    
    Ok(format!("Server started on port {}", port))
}

#[tauri::command]
async fn stop_server(state: State<'_, AppState>) -> Result<String, String> {
    let mut server_guard = state.server.lock().await;
    let mut is_running_guard = state.is_server_running.lock().await;
    
    if !*is_running_guard {
        return Ok("Server is not running".to_string());
    }
    
    *server_guard = None;
    *is_running_guard = false;
    
    Ok("Server stopped".to_string())
}

#[tauri::command]
async fn discover_devices(state: State<'_, AppState>) -> Result<Vec<Device>, String> {
    let discovery_guard = state.network_discovery.lock().await;
    let network_devices = discovery_guard.get_devices();
    
    // Convert network::Device to main::Device
    let devices: Vec<Device> = network_devices.into_iter().map(|d| Device {
        id: d.id,
        name: d.name,
        ip: d.ip,
        port: d.port,
        last_seen: d.last_seen,
    }).collect();
    
    Ok(devices)
}

#[tauri::command]
async fn get_devices(state: State<'_, AppState>) -> Result<Vec<Device>, String> {
    discover_devices(state).await
}

#[tauri::command]
async fn send_file(device_ip: String, file_path: String, state: State<'_, AppState>) -> Result<String, String> {
    let file_handler = state.file_handler.clone();
    let local_ip = local_ip_address::local_ip()
        .unwrap_or_else(|_| std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)));
    let device_name = whoami::fallible::hostname().unwrap_or_else(|_| "Unknown Device".to_string());
    
    // Get the current server port from state
    let server_guard = state.server.lock().await;
    let port = if let Some(server) = server_guard.as_ref() {
        server.port
    } else {
        8080 // fallback port
    };
    drop(server_guard);
    
    match file_handler.send_file_to_device(
        device_ip.clone(),
        port,
        file_path.clone(),
        device_name,
        local_ip.to_string(),
    ).await {
        Ok(result) => Ok(result),
        Err(e) => Err(format!("Failed to send file: {}", e)),
    }
}

#[tauri::command]
async fn get_local_ip() -> Result<String, String> {
    match local_ip_address::local_ip() {
        Ok(ip) => Ok(ip.to_string()),
        Err(e) => Err(format!("Failed to get local IP: {}", e)),
    }
}

#[tauri::command]
async fn get_server_port(state: State<'_, AppState>) -> Result<u16, String> {
    let server_guard = state.server.lock().await;
    if let Some(server) = server_guard.as_ref() {
        Ok(server.port)
    } else {
        Err("Server is not running".to_string())
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
        let _connection = connections_guard.remove(index);
        
        if approved {
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
        let _file = files_guard.remove(index);
        
        if approved {
            if let Some(_path) = save_path {
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

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            pending_connections: Arc::new(Mutex::new(Vec::new())),
            pending_files: Arc::new(Mutex::new(Vec::new())),
            server: Arc::new(Mutex::new(None)),
            network_discovery: Arc::new(Mutex::new(NetworkDiscovery::new())),
            file_handler: Arc::new(FileHandler::new()),
            is_server_running: Arc::new(Mutex::new(false)),
        })
        .invoke_handler(tauri::generate_handler![
            start_server,
            stop_server,
            discover_devices,
            get_devices,
            send_file,
            get_local_ip,
            get_server_port,
            get_pending_connections,
            get_pending_files,
            approve_connection,
            approve_file_transfer
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}