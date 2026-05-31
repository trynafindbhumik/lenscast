use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use rand::Rng;
use std::net::Ipv4Addr;
use std::time::Duration;
use std::process::Command;

const SERVICE_TYPE_PAIRING: &str = "_adb-tls-pairing._tcp.local.";
const SERVICE_TYPE_CONNECT: &str = "_adb-tls-connect._tcp.local.";

pub struct PairService {
    pub service_name: String,
    pub password: String,
    mdns: ServiceDaemon,
}

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub address: Ipv4Addr,
    pub pairing_port: u16,
    pub debugging_port: u16,
}

fn random_number_string(length: usize) -> String {
    let mut rng = rand::rng();
    (0..length)
        .map(|_| rng.random_range(0..10).to_string())
        .collect()
}

impl PairService {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mdns = ServiceDaemon::new()?;
        let service_name = format!("adb-wireless-{}", random_number_string(6));
        let password = random_number_string(8);
        
        Ok(Self {
            service_name,
            password,
            mdns,
        })
    }

    pub fn qr_text(&self) -> String {
        format!("WIFI:T:ADB;S:{};P:{};;", self.service_name, self.password)
    }

    pub fn start_discovery(&self) -> Result<(), Box<dyn std::error::Error>> {
        let service_info = ServiceInfo::new(
            SERVICE_TYPE_PAIRING,
            &self.service_name,
            &format!("{}.local.", self.service_name),
            "",
            0,
            None,
        )?;
        self.mdns.register(service_info)?;
        Ok(())
    }

    pub fn wait_for_pairing(&self) -> Result<DeviceInfo, Box<dyn std::error::Error>> {
        // Step 1: Browse for pairing service response
        let receiver = self.mdns.browse(SERVICE_TYPE_PAIRING)?;
        
        let (address, pairing_port) = loop {
            match receiver.recv() {
                Ok(ServiceEvent::ServiceResolved(info)) => {
                    if info.get_fullname().contains(&self.service_name) {
                        let client_addresses = info.get_addresses_v4();
                        let port = info.get_port();
                        
                        let _ = self.mdns.stop_browse(SERVICE_TYPE_PAIRING);
                        
                        // Filter for private IP and break
                        if let Some(addr) = client_addresses
                            .iter()
                            .find(|addr| addr.is_private())
                        {
                            break (**addr, port);
                        } else {
                            return Err("No private client address found".into());
                        }
                    }
                }
                Ok(_) => std::thread::sleep(Duration::from_millis(100)),
                Err(err) => {
                    let _ = self.mdns.stop_browse(SERVICE_TYPE_PAIRING);
                    return Err(format!("mDNS browse error: {}", err).into());
                }
            }
        };

        // Step 2: Browse for connect service (debugging port)
        let receiver = self.mdns.browse(SERVICE_TYPE_CONNECT)?;
        let start_time = std::time::Instant::now();
        let timeout = Duration::from_secs(15);

        let debugging_port = loop {
            if start_time.elapsed() > timeout {
                return Err("Timeout waiting for debugging port".into());
            }

            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(ServiceEvent::ServiceResolved(info)) => {
                    let port = info.get_port();
                    
                    // ✅ FIXED: Double dereference to match original logic
                    if info.get_addresses_v4().iter().any(|&&addr| addr == address) {
                        let _ = self.mdns.stop_browse(SERVICE_TYPE_CONNECT);
                        break port;
                    } else {
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
                Ok(_) => std::thread::sleep(Duration::from_millis(100)),
                Err(flume::RecvTimeoutError::Timeout) => continue,
                Err(err) => {
                    let _ = self.mdns.stop_browse(SERVICE_TYPE_CONNECT);
                    return Err(format!("mDNS connect error: {}", err).into());
                }
            }
        };

        Ok(DeviceInfo {
            address,
            pairing_port,
            debugging_port,
        })
    }

    pub fn execute_pair_and_connect(
        device: &DeviceInfo,
        password: &str,
    ) -> Result<DeviceInfo, String> {
        // adb pair
        let pair_output = Command::new("adb")
            .args([
                "pair",
                &format!("{}:{}", device.address, device.pairing_port),
                password,
            ])
            .output()
            .map_err(|e| format!("adb pair failed: {}", e))?;

        if !pair_output.status.success() {
            return Err(format!(
                "adb pair failed: {}",
                String::from_utf8_lossy(&pair_output.stderr).trim()
            ));
        }

        // adb connect
        let connect_output = Command::new("adb")
            .args([
                "connect",
                &format!("{}:{}", device.address, device.debugging_port),
            ])
            .output()
            .map_err(|e| format!("adb connect failed: {}", e))?;

        if !connect_output.status.success() {
            return Err(format!(
                "adb connect failed: {}",
                String::from_utf8_lossy(&connect_output.stderr).trim()
            ));
        }

        Ok(device.clone())
    }
}

impl Drop for PairService {
    fn drop(&mut self) {
        let _ = self.mdns.stop_browse(SERVICE_TYPE_PAIRING);
        let _ = self.mdns.stop_browse(SERVICE_TYPE_CONNECT);
        let _ = self.mdns.unregister(&self.service_name);
    }
}