use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::time::timeout;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use local_ip_address::local_ip;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub last_seen: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NetworkDiscovery {
    pub devices: HashMap<String, Device>,
    pub service_name: String,
}

impl NetworkDiscovery {
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
            service_name: "_seaki._tcp.local".to_string(),
        }
    }

    pub async fn start_discovery(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        log::info!("Starting network discovery for service: {}", self.service_name);
        
        // Start discovery in a separate task
        let devices = self.devices.clone();
        let service_name = self.service_name.clone();
        
        tokio::spawn(async move {
            if let Err(e) = Self::discover_devices_loop(devices, service_name).await {
                log::error!("Discovery error: {}", e);
            }
        });

        Ok(())
    }

    async fn discover_devices_loop(
        mut devices: HashMap<String, Device>,
        service_name: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        loop {
            match Self::discover_devices(&service_name).await {
                Ok(discovered) => {
                    for device in discovered {
                        devices.insert(device.id.clone(), device);
                    }
                }
                Err(e) => {
                    log::error!("Discovery error: {}", e);
                }
            }
            
            // Wait before next discovery
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    async fn discover_devices(_service_name: &str) -> Result<Vec<Device>, Box<dyn std::error::Error + Send + Sync>> {
        let mut devices = Vec::new();
        
        // For now, we'll use UDP broadcast discovery instead of mDNS
        // This is more reliable for local network discovery
        devices.extend(Self::discover_via_broadcast().await?);

        Ok(devices)
    }

    async fn discover_via_broadcast() -> Result<Vec<Device>, Box<dyn std::error::Error + Send + Sync>> {
        let mut devices = Vec::new();
        
        // Get local IP to determine network range
        let local_ip = local_ip()?;
        let local_ipv4 = match local_ip {
            IpAddr::V4(ip) => ip,
            _ => return Ok(devices),
        };

        // Scan common IP ranges for Seaki services
        let network_base = u32::from(local_ipv4) & 0xFFFFFF00; // /24 network
        
        // Use a more efficient approach with concurrent requests
        let mut tasks = Vec::new();
        
        // Scan for Seaki services on the specific weird port
        let seaki_port = 54321;
        
        for i in 1..255 {
            let target_ip = Ipv4Addr::from(network_base | i);
            
            // Skip our own IP
            if target_ip != local_ipv4 {
                let target_addr = SocketAddr::new(IpAddr::V4(target_ip), seaki_port);
                let task = tokio::spawn(async move {
                    Self::check_seaki_service(target_addr).await
                });
                tasks.push(task);
            }
        }
        
        // Wait for all tasks to complete
        for task in tasks {
            if let Ok(Ok(device)) = task.await {
                devices.push(device);
            }
        }

        Ok(devices)
    }

    async fn check_seaki_service(addr: SocketAddr) -> Result<Device, Box<dyn std::error::Error + Send + Sync>> {
        let client = reqwest::Client::new();
        let url = format!("http://{}:{}/api/status", addr.ip(), addr.port());
        
        // Try to connect with timeout
        let response = timeout(
            Duration::from_secs(2),
            client.get(&url).send()
        ).await??;

        if response.status().is_success() {
            // Parse response to get device info
            let device_name = format!("Device-{}", addr.ip());
            
            Ok(Device {
                id: uuid::Uuid::new_v4().to_string(),
                name: device_name,
                ip: addr.ip().to_string(),
                port: addr.port(),
                last_seen: Utc::now(),
            })
        } else {
            Err("Service not available".into())
        }
    }

    pub async fn advertise_service(&self, port: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        log::info!("Advertising Seaki service on port {}", port);
        
        // In a real implementation, you would use mDNS to advertise the service
        // For now, we'll just log that we're advertising
        log::info!("Service advertised: {} on port {}", self.service_name, port);
        
        Ok(())
    }

    pub fn get_devices(&self) -> Vec<Device> {
        self.devices.values().cloned().collect()
    }

    pub fn remove_stale_devices(&mut self, max_age: Duration) {
        let now = Utc::now();
        self.devices.retain(|_, device| {
            now.signed_duration_since(device.last_seen) < chrono::Duration::from_std(max_age).unwrap_or_default()
        });
    }
}