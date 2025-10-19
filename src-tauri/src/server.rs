use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::filters::multipart::FormData;
use warp::http::StatusCode;
use warp::reply::{json, with_status};
use warp::{Filter, Reply, Buf};
use serde::{Deserialize, Serialize};
use futures_util::stream::StreamExt;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::fs;
use tempfile::TempDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionRequest {
    pub device_name: String,
    pub device_ip: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferRequest {
    pub file_name: String,
    pub file_size: u64,
    pub file_hash: String,
    pub sender_name: String,
    pub sender_ip: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub hash: String,
    pub mime_type: String,
    pub created_at: DateTime<Utc>,
}

pub struct FileServer {
    pub port: u16,
    pub temp_dir: Arc<TempDir>,
    pub files: Arc<Mutex<HashMap<String, FileInfo>>>,
    pub pending_connections: Arc<Mutex<Vec<ConnectionRequest>>>,
    pub pending_files: Arc<Mutex<Vec<FileTransferRequest>>>,
}

impl FileServer {
    pub fn new(port: u16) -> Self {
        let temp_dir = Arc::new(TempDir::new().expect("Failed to create temp directory"));
        
        Self {
            port,
            temp_dir,
            files: Arc::new(Mutex::new(HashMap::new())),
            pending_connections: Arc::new(Mutex::new(Vec::new())),
            pending_files: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let files = self.files.clone();
        let temp_dir = self.temp_dir.clone();
        let pending_connections = self.pending_connections.clone();
        let pending_files = self.pending_files.clone();

        // CORS headers
        let cors = warp::cors()
            .allow_any_origin()
            .allow_headers(vec!["content-type"])
            .allow_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"]);

        // Status endpoint
        let status = warp::path("api")
            .and(warp::path("status"))
            .and(warp::get())
            .map(|| {
                json(&serde_json::json!({
                    "status": "running",
                    "timestamp": Utc::now()
                }))
            });

        // Connection request endpoint
        let connection_request = warp::path("api")
            .and(warp::path("connect"))
            .and(warp::post())
            .and(warp::body::json())
            .and(with_state(pending_connections.clone()))
            .and_then(handle_connection_request);

        // File transfer request endpoint
        let file_transfer_request = warp::path("api")
            .and(warp::path("transfer"))
            .and(warp::post())
            .and(warp::body::json())
            .and(with_state(pending_files.clone()))
            .and_then(handle_file_transfer_request);

        // File upload endpoint
        let upload = warp::path("api")
            .and(warp::path("upload"))
            .and(warp::post())
            .and(warp::multipart::form().max_length(1024 * 1024 * 1024)) // 1GB max
            .and(with_state(files.clone()))
            .and(with_state(temp_dir.clone()))
            .and_then(handle_file_upload);

        // File download endpoint
        let download = warp::path("api")
            .and(warp::path("download"))
            .and(warp::path::param::<String>())
            .and(warp::get())
            .and(with_state(files.clone()))
            .and(with_state(temp_dir.clone()))
            .and_then(handle_file_download);

        // File list endpoint
        let file_list = warp::path("api")
            .and(warp::path("files"))
            .and(warp::get())
            .and(with_state(files.clone()))
            .and_then(handle_file_list);

        let routes = status
            .or(connection_request)
            .or(file_transfer_request)
            .or(upload)
            .or(download)
            .or(file_list)
            .with(cors);

        let addr = ([0, 0, 0, 0], self.port);
        log::info!("Starting HTTP server on port {}", self.port);
        
        warp::serve(routes)
            .run(addr)
            .await;

        Ok(())
    }
}

async fn handle_connection_request(
    request: ConnectionRequest,
    pending_connections: Arc<Mutex<Vec<ConnectionRequest>>>,
) -> Result<impl Reply, warp::Rejection> {
    log::info!("Connection request from {} ({})", request.device_name, request.device_ip);
    
    let mut connections = pending_connections.lock().await;
    connections.push(request);
    
    Ok(with_status(
        json(&serde_json::json!({
            "status": "connection_request_received"
        })),
        StatusCode::OK,
    ))
}

async fn handle_file_transfer_request(
    request: FileTransferRequest,
    pending_files: Arc<Mutex<Vec<FileTransferRequest>>>,
) -> Result<impl Reply, warp::Rejection> {
    log::info!("File transfer request: {} from {}", request.file_name, request.sender_name);
    
    let mut files = pending_files.lock().await;
    files.push(request);
    
    Ok(with_status(
        json(&serde_json::json!({
            "status": "file_transfer_request_received"
        })),
        StatusCode::OK,
    ))
}

async fn handle_file_upload(
    mut form: FormData,
    files: Arc<Mutex<HashMap<String, FileInfo>>>,
    temp_dir: Arc<TempDir>,
) -> Result<impl Reply, warp::Rejection> {
    let mut file_data = Vec::new();
    let mut file_name = String::new();
    let mut file_size = 0u64;

    while let Some(part) = form.next().await {
        let part = part.map_err(|e| {
            log::error!("Error processing form part: {}", e);
            warp::reject::custom(FileUploadError)
        })?;

        if part.name() == "file" {
            file_name = part.filename().unwrap_or("unknown").to_string();
            
            let mut stream = part.stream();
            while let Some(chunk) = stream.next().await {
                let mut chunk = chunk.map_err(|e| {
                    log::error!("Error reading chunk: {}", e);
                    warp::reject::custom(FileUploadError)
                })?;
                
                // Convert Buf to bytes
                let chunk_data = chunk.copy_to_bytes(chunk.remaining());
                file_size += chunk_data.len() as u64;
                file_data.extend_from_slice(&chunk_data);
            }
        }
    }

    if file_data.is_empty() {
        return Ok(with_status(
            json(&serde_json::json!({
                "error": "No file data received"
            })),
            StatusCode::BAD_REQUEST,
        ));
    }

    // Generate file ID and save to temp directory
    let file_id = Uuid::new_v4().to_string();
    let file_path = temp_dir.path().join(&file_id);
    
    if let Err(e) = fs::write(&file_path, &file_data) {
        log::error!("Failed to write file: {}", e);
        return Ok(with_status(
            json(&serde_json::json!({
                "error": "Failed to save file"
            })),
            StatusCode::INTERNAL_SERVER_ERROR,
        ));
    }

    // Calculate file hash
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(&file_data);
    let file_hash = hex::encode(hasher.finalize());

    // Get MIME type
    let mime_type = mime_guess::from_path(&file_name)
        .first_or_octet_stream()
        .to_string();

    // Store file info
    let file_info = FileInfo {
        id: file_id.clone(),
        name: file_name.clone(),
        size: file_size,
        hash: file_hash,
        mime_type,
        created_at: Utc::now(),
    };

    {
        let mut files_map = files.lock().await;
        files_map.insert(file_id.clone(), file_info);
    }

    log::info!("File uploaded successfully: {} ({} bytes)", file_name, file_size);

    Ok(with_status(
        json(&serde_json::json!({
            "file_id": file_id,
            "status": "uploaded"
        })),
        StatusCode::OK,
    ))
}

async fn handle_file_download(
    file_id: String,
    files: Arc<Mutex<HashMap<String, FileInfo>>>,
    temp_dir: Arc<TempDir>,
) -> Result<impl Reply, warp::Rejection> {
    let file_info = {
        let files_map = files.lock().await;
        files_map.get(&file_id).cloned()
    };

    match file_info {
        Some(info) => {
            let file_path = temp_dir.path().join(&file_id);
            
            match fs::read(&file_path) {
                Ok(file_data) => {
                    log::info!("Serving file: {} ({} bytes)", info.name, info.size);
                    Ok(warp::reply::with_header(
                        file_data,
                        "Content-Type",
                        info.mime_type,
                    ).into_response())
                }
                Err(e) => {
                    log::error!("Failed to read file: {}", e);
                    Ok(with_status(
                        json(&serde_json::json!({
                            "error": "File not found"
                        })),
                        StatusCode::NOT_FOUND,
                    ).into_response())
                }
            }
        }
        None => {
            Ok(with_status(
                json(&serde_json::json!({
                    "error": "File not found"
                })),
                StatusCode::NOT_FOUND,
            ).into_response())
        }
    }
}

async fn handle_file_list(
    files: Arc<Mutex<HashMap<String, FileInfo>>>,
) -> Result<impl Reply, warp::Rejection> {
    let files_map = files.lock().await;
    let file_list: Vec<FileInfo> = files_map.values().cloned().collect();
    
    Ok(json(&file_list))
}

// Helper function to add state to warp filters
fn with_state<T: Clone + Send + Sync>(
    state: T,
) -> impl Filter<Extract = (T,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || state.clone())
}

#[derive(Debug)]
struct FileUploadError;

impl warp::reject::Reject for FileUploadError {}