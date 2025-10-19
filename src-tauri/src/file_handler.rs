use std::path::Path;
use std::fs;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use reqwest::Client;
use sha2::{Sha256, Digest};
use hex;
use mime_guess;

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
pub struct ConnectionRequest {
    pub device_name: String,
    pub device_ip: String,
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

pub struct FileHandler {
    client: Client,
}

impl FileHandler {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub async fn send_file_to_device(
        &self,
        device_ip: String,
        device_port: u16,
        file_path: String,
        sender_name: String,
        sender_ip: String,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        log::info!("Sending file {} to {}:{}", file_path, device_ip, device_port);

        // Read file
        let file_data = fs::read(&file_path)?;
        let file_name = Path::new(&file_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Calculate file hash
        let file_hash = self.calculate_file_hash(&file_data);

        // Get MIME type
        let mime_type = mime_guess::from_path(&file_path)
            .first_or_octet_stream()
            .to_string();

        // First, send connection request
        self.send_connection_request(&device_ip, device_port, &sender_name, &sender_ip).await?;

        // Wait a bit for connection approval (in real implementation, this would be async)
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        // Send file transfer request
        self.send_file_transfer_request(
            &device_ip,
            device_port,
            &file_name,
            file_data.len() as u64,
            &file_hash,
            &sender_name,
            &sender_ip,
        ).await?;

        // Wait for file transfer approval
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        // Upload the actual file
        self.upload_file(&device_ip, device_port, &file_name, &file_data, &mime_type).await?;

        Ok(format!("File {} sent successfully to {}", file_name, device_ip))
    }

    async fn send_connection_request(
        &self,
        device_ip: &str,
        device_port: u16,
        sender_name: &str,
        sender_ip: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("http://{}:{}/api/connect", device_ip, device_port);
        
        let request = ConnectionRequest {
            device_name: sender_name.to_string(),
            device_ip: sender_ip.to_string(),
            timestamp: Utc::now(),
        };

        let response = self.client
            .post(&url)
            .json(&request)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("Connection request failed: {}", response.status()).into());
        }

        log::info!("Connection request sent to {}:{}", device_ip, device_port);
        Ok(())
    }

    async fn send_file_transfer_request(
        &self,
        device_ip: &str,
        device_port: u16,
        file_name: &str,
        file_size: u64,
        file_hash: &str,
        sender_name: &str,
        sender_ip: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("http://{}:{}/api/transfer", device_ip, device_port);
        
        let request = FileTransferRequest {
            file_name: file_name.to_string(),
            file_size,
            file_hash: file_hash.to_string(),
            sender_name: sender_name.to_string(),
            sender_ip: sender_ip.to_string(),
            timestamp: Utc::now(),
        };

        let response = self.client
            .post(&url)
            .json(&request)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("File transfer request failed: {}", response.status()).into());
        }

        log::info!("File transfer request sent to {}:{}", device_ip, device_port);
        Ok(())
    }

    async fn upload_file(
        &self,
        device_ip: &str,
        device_port: u16,
        file_name: &str,
        file_data: &[u8],
        mime_type: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("http://{}:{}/api/upload", device_ip, device_port);
        
        // Create multipart form
        let form = reqwest::multipart::Form::new()
            .part("file", reqwest::multipart::Part::bytes(file_data.to_vec())
                .file_name(file_name.to_string())
                .mime_str(mime_type)?);

        let response = self.client
            .post(&url)
            .multipart(form)
            .timeout(std::time::Duration::from_secs(60)) // 1 minute timeout for file upload
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("File upload failed: {}", response.status()).into());
        }

        log::info!("File uploaded successfully to {}:{}", device_ip, device_port);
        Ok(())
    }

    fn calculate_file_hash(&self, data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    pub async fn download_file(
        &self,
        device_ip: String,
        device_port: u16,
        file_id: String,
        save_path: String,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        log::info!("Downloading file {} from {}:{}", file_id, device_ip, device_port);

        let url = format!("http://{}:{}/api/download/{}", device_ip, device_port, file_id);
        
        let response = self.client
            .get(&url)
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("File download failed: {}", response.status()).into());
        }

        let file_data = response.bytes().await?;
        
        // Ensure directory exists
        if let Some(parent) = Path::new(&save_path).parent() {
            fs::create_dir_all(parent)?;
        }

        // Write file
        fs::write(&save_path, &file_data)?;

        log::info!("File downloaded successfully to {}", save_path);
        Ok(format!("File downloaded to {}", save_path))
    }

    pub async fn get_file_list(
        &self,
        device_ip: String,
        device_port: u16,
    ) -> Result<Vec<FileInfo>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("http://{}:{}/api/files", device_ip, device_port);
        
        let response = self.client
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("Failed to get file list: {}", response.status()).into());
        }

        let files: Vec<FileInfo> = response.json().await?;
        Ok(files)
    }
}