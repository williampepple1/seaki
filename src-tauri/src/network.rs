use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub last_seen: chrono::DateTime<chrono::Utc>,
}

impl Device {
    pub fn new(name: String, ip: String, port: u16) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            ip,
            port,
            last_seen: chrono::Utc::now(),
        }
    }
}

pub async fn discover_devices() -> Result<Vec<Device>, String> {
    let mut devices = Vec::new();
    
    // Get local IP range
    let local_ip = match local_ip_address::local_ip() {
        Ok(ip) => ip,
        Err(e) => return Err(format!("Failed to get local IP: {}", e)),
    };

    if let IpAddr::V4(ipv4) = local_ip {
        let base_ip = format!("{}.{}.{}.", ipv4.octets()[0], ipv4.octets()[1], ipv4.octets()[2]);
        
        // Scan common ports for file sharing services
        let ports = vec![8080, 8081, 8082, 3000, 5000];
        
        for port in ports {
            // Scan IPs in the local network range
            for i in 1..255 {
                let target_ip = format!("{}{}", base_ip, i);
                
                if let Ok(ip_addr) = target_ip.parse::<IpAddr>() {
                    if let Some(device) = check_device(ip_addr, port).await {
                        devices.push(device);
                    }
                }
            }
        }
    }

    // Also try mDNS discovery
    if let Ok(mdns_devices) = discover_via_mdns().await {
        devices.extend(mdns_devices);
    }

    Ok(devices)
}

async fn check_device(ip: IpAddr, port: u16) -> Option<Device> {
    let url = format!("http://{}:{}/api/status", ip, port);
    
    match timeout(Duration::from_millis(1000), reqwest::get(&url)).await {
        Ok(Ok(response)) if response.status().is_success() => {
            if let Ok(device_info) = response.json::<serde_json::Value>().await {
                let name = device_info
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown Device")
                    .to_string();
                
                Some(Device::new(name, ip.to_string(), port))
            } else {
                None
            }
        }
        _ => None,
    }
}

async fn discover_via_mdns() -> Result<Vec<Device>, String> {
    let mut devices = Vec::new();
    
    // Use mDNS to discover services
    let service = "_seaki._tcp.local";
    
    match mdns::discover::all(service, Duration::from_secs(5)) {
        Ok(stream) => {
            for response in stream {
                if let Ok(response) = response {
                    for service in response.services() {
                        let name = service.name().to_string();
                        let ip = service.ipv4_addresses()
                            .first()
                            .map(|ip| ip.to_string())
                            .unwrap_or_default();
                        let port = service.port();
                        
                        if !ip.is_empty() {
                            devices.push(Device::new(name, ip, port));
                        }
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("mDNS discovery error: {}", e);
        }
    }
    
    Ok(devices)
}

pub async fn advertise_service(port: u16, device_name: String) -> Result<(), String> {
    let service = mdns::Service::new("_seaki._tcp.local", &device_name, port);
    
    match service.advertise() {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Failed to advertise service: {}", e)),
    }
}
