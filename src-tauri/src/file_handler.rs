use std::path::Path;
use tokio::fs;

pub async fn send_file_to_device(device_ip: &str, file_path: &str) -> Result<String, String> {
    let file_path = Path::new(file_path);
    
    if !file_path.exists() {
        return Err("File does not exist".to_string());
    }

    let file_name = file_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Invalid file name")?;

    let file_data = fs::read(file_path)
        .await
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let client = reqwest::Client::new();
    let url = format!("http://{}:8080/api/upload", device_ip);

    let form = reqwest::multipart::Form::new()
        .part("file", reqwest::multipart::Part::bytes(file_data).file_name(file_name.to_string()));

    match client.post(&url).multipart(form).send().await {
        Ok(response) => {
            if response.status().is_success() {
                Ok("File sent successfully".to_string())
            } else {
                Err(format!("Server returned error: {}", response.status()))
            }
        }
        Err(e) => Err(format!("Failed to send file: {}", e)),
    }
}

pub async fn get_file_info(device_ip: &str, file_id: &str) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let url = format!("http://{}:8080/api/status", device_ip);

    match client.get(&url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                response.json::<serde_json::Value>()
                    .await
                    .map_err(|e| format!("Failed to parse response: {}", e))
            } else {
                Err(format!("Server returned error: {}", response.status()))
            }
        }
        Err(e) => Err(format!("Failed to get file info: {}", e)),
    }
}

pub async fn download_file(device_ip: &str, file_id: &str, save_path: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let url = format!("http://{}:8080/api/download/{}", device_ip, file_id);

    match client.get(&url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                let data = response.bytes().await
                    .map_err(|e| format!("Failed to read response: {}", e))?;
                
                fs::write(save_path, data)
                    .await
                    .map_err(|e| format!("Failed to save file: {}", e))?;
                
                Ok("File downloaded successfully".to_string())
            } else {
                Err(format!("Server returned error: {}", response.status()))
            }
        }
        Err(e) => Err(format!("Failed to download file: {}", e)),
    }
}
