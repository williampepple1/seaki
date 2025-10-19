use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Mutex;
use warp::Filter;

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

pub struct FileServer {
    pub port: u16,
    pub device_name: String,
    pub files: Arc<Mutex<HashMap<String, FileInfo>>>,
}

impl FileServer {
    pub async fn new() -> Result<Self, String> {
        let device_name = whoami::hostname();
        let port = 8080; // Default port
        
        Ok(Self {
            port,
            device_name,
            files: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn start(&self) -> Result<(), String> {
        let files = self.files.clone();
        let device_name = self.device_name.clone();
        let port = self.port;

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

        let routes = status.or(upload).or(download);

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
