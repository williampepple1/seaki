use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Mutex;
use warp::Filter;
use uuid::Uuid;
use chrono::Utc;
use std::net::IpAddr;
use warp::reject::Rejection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub mime_type: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStatus {
    pub name: String,
    pub status: String,
    pub files: Vec<FileInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionRequest {
    pub device_name: String,
    pub device_ip: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferRequest {
    pub file_name: String,
    pub file_size: u64,
    pub sender_name: String,
    pub sender_ip: String,
}

pub struct FileServer {
    pub port: u16,
    pub device_name: String,
    pub files: Arc<Mutex<HashMap<String, FileInfo>>>,
    pub pending_connections: Arc<Mutex<Vec<crate::IncomingConnection>>>,
    pub pending_files: Arc<Mutex<Vec<crate::IncomingFile>>>,
}

impl FileServer {
    pub async fn new() -> Result<Self, String> {
        let device_name = whoami::hostname();
        let port = 8080; // Default port
        
        Ok(Self {
            port,
            device_name,
            files: Arc::new(Mutex::new(HashMap::new())),
            pending_connections: Arc::new(Mutex::new(Vec::new())),
            pending_files: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub async fn start(&self) -> Result<(), String> {
        let files = self.files.clone();
        let device_name = self.device_name.clone();
        let port = self.port;
        
        // Get local IP for network validation
        let local_ip = match local_ip_address::local_ip() {
            Ok(ip) => ip,
            Err(_) => return Err("Failed to get local IP".to_string()),
        };

        // API routes
        let status = warp::path("api")
            .and(warp::path("status"))
            .and(warp::get())
            .map(move || {
                let files_guard = files.blocking_lock();
                let files_list: Vec<FileInfo> = files_guard.values().cloned().collect();
                
                warp::reply::json(&ServerStatus {
                    name: device_name.clone(),
                    status: "online".to_string(),
                    files: files_list,
                })
            });

        let upload = warp::path("api")
            .and(warp::path("upload"))
            .and(warp::post())
            .and(warp::multipart::form().max_length(1024 * 1024 * 1024)) // 1GB max
            .and_then(move |form: warp::multipart::FormData| {
                let files = files.clone();
                async move {
                    handle_upload(form, files).await
                }
            });

        let download = warp::path("api")
            .and(warp::path("download"))
            .and(warp::path::param::<String>())
            .and(warp::get())
            .and_then(move |file_id: String| {
                let files = files.clone();
                async move {
                    handle_download(file_id, files).await
                }
            });

        let connection_request = warp::path("api")
            .and(warp::path("connect"))
            .and(warp::post())
            .and(warp::body::json())
            .and_then(move |request: ConnectionRequest| {
                let pending_connections = self.pending_connections.clone();
                async move {
                    handle_connection_request(request, pending_connections).await
                }
            });

        let file_transfer_request = warp::path("api")
            .and(warp::path("transfer"))
            .and(warp::post())
            .and(warp::body::json())
            .and_then(move |request: FileTransferRequest| {
                let pending_files = self.pending_files.clone();
                async move {
                    handle_file_transfer_request(request, pending_files).await
                }
            });

        // Add network validation middleware
        let network_filter = warp::any()
            .and(warp::header::optional::<String>("x-forwarded-for"))
            .and(warp::header::optional::<String>("x-real-ip"))
            .map(|forwarded: Option<String>, real_ip: Option<String>| {
                // Extract client IP from headers or use remote address
                let client_ip = forwarded
                    .or(real_ip)
                    .unwrap_or_else(|| "127.0.0.1".to_string());
                client_ip
            })
            .and_then(move |client_ip: String| {
                let local_ip = local_ip.clone();
                async move {
                    // Check if client is from same network
                    if is_same_network(&client_ip, &local_ip) {
                        Ok(client_ip)
                    } else {
                        Err(warp::reject::custom(NetworkRejection))
                    }
                }
            });

        let routes = status
            .or(upload)
            .or(download)
            .or(connection_request)
            .or(file_transfer_request)
            .with(network_filter);

        println!("Starting server on port {}", port);
        warp::serve(routes)
            .run(([0, 0, 0, 0], port))
            .await;

        Ok(())
    }
}

async fn handle_upload(
    mut form: warp::multipart::FormData,
    files: Arc<Mutex<HashMap<String, FileInfo>>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    use warp::multipart::Part;

    while let Some(part) = form.next().await {
        if let Ok(part) = part {
            if let Some(filename) = part.filename() {
                let mut data = Vec::new();
                let mut stream = part.stream();
                
                while let Some(chunk) = stream.next().await {
                    if let Ok(chunk) = chunk {
                        data.extend_from_slice(&chunk);
                    }
                }

                let file_size = data.len() as u64;
                let mime_type = mime_guess::from_path(filename).first_or_text_plain().to_string();
                
                // Generate file hash
                use sha2::{Sha256, Digest};
                let mut hasher = Sha256::new();
                hasher.update(&data);
                let hash = hex::encode(hasher.finalize());

                let file_id = uuid::Uuid::new_v4().to_string();
                let file_info = FileInfo {
                    name: filename.to_string(),
                    size: file_size,
                    mime_type,
                    hash,
                };

                // Store file info
                {
                    let mut files_guard = files.lock().await;
                    files_guard.insert(file_id.clone(), file_info);
                }

                // Save file to disk
                let file_path = format!("./uploads/{}", file_id);
                if let Err(e) = fs::create_dir_all("./uploads").await {
                    eprintln!("Failed to create uploads directory: {}", e);
                }
                
                if let Err(e) = fs::write(&file_path, data).await {
                    eprintln!("Failed to save file: {}", e);
                }

                return Ok(warp::reply::json(&serde_json::json!({
                    "success": true,
                    "file_id": file_id,
                    "message": "File uploaded successfully"
                })));
            }
        }
    }

    Ok(warp::reply::json(&serde_json::json!({
        "success": false,
        "message": "No file received"
    })))
}

async fn handle_download(
    file_id: String,
    files: Arc<Mutex<HashMap<String, FileInfo>>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let file_info = {
        let files_guard = files.lock().await;
        files_guard.get(&file_id).cloned()
    };

    match file_info {
        Some(info) => {
            let file_path = format!("./uploads/{}", file_id);
            
            match fs::read(&file_path).await {
                Ok(data) => {
                    let response = warp::reply::Response::new(data.into());
                    Ok(warp::reply::with_header(
                        response,
                        "Content-Disposition",
                        format!("attachment; filename=\"{}\"", info.name),
                    ))
                }
                Err(_) => {
                    Ok(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({
                            "error": "File not found"
                        })),
                        warp::http::StatusCode::NOT_FOUND,
                    ))
                }
            }
        }
        None => {
            Ok(warp::reply::with_status(
                warp::reply::json(&serde_json::json!({
                    "error": "File not found"
                })),
                warp::http::StatusCode::NOT_FOUND,
            ))
        }
    }
}

async fn handle_connection_request(
    request: ConnectionRequest,
    pending_connections: Arc<Mutex<Vec<crate::IncomingConnection>>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let connection = crate::IncomingConnection {
        id: Uuid::new_v4().to_string(),
        device_name: request.device_name,
        device_ip: request.device_ip,
        timestamp: Utc::now(),
    };

    {
        let mut connections_guard = pending_connections.lock().await;
        connections_guard.push(connection.clone());
    }

    // TODO: Send notification to UI
    println!("New connection request from: {} ({})", connection.device_name, connection.device_ip);

    Ok(warp::reply::json(&serde_json::json!({
        "success": true,
        "message": "Connection request sent",
        "connection_id": connection.id
    })))
}

async fn handle_file_transfer_request(
    request: FileTransferRequest,
    pending_files: Arc<Mutex<Vec<crate::IncomingFile>>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let file = crate::IncomingFile {
        id: Uuid::new_v4().to_string(),
        file_name: request.file_name,
        file_size: request.file_size,
        sender_name: request.sender_name,
        sender_ip: request.sender_ip,
        timestamp: Utc::now(),
    };

    {
        let mut files_guard = pending_files.lock().await;
        files_guard.push(file.clone());
    }

    // TODO: Send notification to UI
    println!("New file transfer request: {} from {} ({})", 
        file.file_name, file.sender_name, file.sender_ip);

    Ok(warp::reply::json(&serde_json::json!({
        "success": true,
        "message": "File transfer request sent",
        "file_id": file.id
    })))
}

// Network security functions
#[derive(Debug)]
struct NetworkRejection;

impl warp::reject::Reject for NetworkRejection {}

fn is_same_network(client_ip: &str, local_ip: &IpAddr) -> bool {
    // Parse client IP
    let client_ip: IpAddr = match client_ip.parse() {
        Ok(ip) => ip,
        Err(_) => return false,
    };

    // Allow localhost connections
    if client_ip.is_loopback() {
        return true;
    }

    // Check if both IPs are IPv4 and in same subnet
    if let (IpAddr::V4(client_v4), IpAddr::V4(local_v4)) = (client_ip, local_ip) {
        // Check if they're in the same /24 subnet (same network)
        let client_network = u32::from(client_v4) & 0xFFFFFF00; // /24 mask
        let local_network = u32::from(local_v4) & 0xFFFFFF00; // /24 mask
        
        return client_network == local_network;
    }

    // For IPv6, check if they're in the same /64 subnet
    if let (IpAddr::V6(client_v6), IpAddr::V6(local_v6)) = (client_ip, local_ip) {
        let client_bytes = client_v6.octets();
        let local_bytes = local_v6.octets();
        
        // Check first 8 bytes (64 bits) are the same
        return client_bytes[..8] == local_bytes[..8];
    }

    false
}
