use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use rand::Rng;
use std::net::Ipv4Addr;
use std::process::Command;
use std::time::Duration;

const SERVICE_TYPE_PAIRING: &str = "_adb-tls-pairing._tcp.local.";
const SERVICE_TYPE_CONNECT: &str = "_adb-tls-connect._tcp.local.";

/// Service for wireless ADB pairing using mDNS.
pub struct PairService {
    pub service_name: String,
    pub password: String,
    mdns: ServiceDaemon,
}

/// Device information discovered during pairing.
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

    /// Returns the WiFi pairing QR code payload string.
    pub fn qr_text(&self) -> String {
        format!("WIFI:T:ADB;S:{};P:{};;", self.service_name, self.password)
    }

    /// Registers the pairing service on the local network.
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

    /// Waits for a device to connect and returns its address and ports.
    pub fn wait_for_pairing(&self) -> Result<DeviceInfo, Box<dyn std::error::Error>> {
        let receiver = self.mdns.browse(SERVICE_TYPE_PAIRING)?;

        let (address, pairing_port) = loop {
            match receiver.recv() {
                Ok(ServiceEvent::ServiceResolved(info)) => {
                    if info.get_fullname().contains(&self.service_name) {
                        let client_addresses = info.get_addresses_v4();
                        let port = info.get_port();

                        let _ = self.mdns.stop_browse(SERVICE_TYPE_PAIRING);

                        if let Some(addr) = client_addresses.iter().find(|addr| addr.is_private()) {
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

        // Browse for connect service to get debugging port
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

    pub fn discover_device_for_connect() -> Result<DeviceInfo, Box<dyn std::error::Error>> {
        let mdns = ServiceDaemon::new()?;
        let receiver = mdns.browse(SERVICE_TYPE_CONNECT)?;
        let start_time = std::time::Instant::now();
        let timeout = Duration::from_secs(10);

        loop {
            if start_time.elapsed() > timeout {
                return Err("No connect service discovered (timeout)".into());
            }

            match receiver.recv_timeout(Duration::from_millis(500)) {
                Ok(ServiceEvent::ServiceResolved(info)) => {
                    let addresses = info.get_addresses_v4();
                    let port = info.get_port();

                    if let Some(addr) = addresses.iter().find(|addr| addr.is_private()) {
                        return Ok(DeviceInfo {
                            address: **addr,
                            pairing_port: 0,
                            debugging_port: port,
                        });
                    }
                }
                Ok(_) => {}
                Err(flume::RecvTimeoutError::Timeout) => continue,
                Err(flume::RecvTimeoutError::Disconnected) => {
                    std::thread::sleep(Duration::from_millis(500));
                    let receiver = mdns.browse(SERVICE_TYPE_CONNECT)?;
                    let _ = receiver.recv_timeout(Duration::from_millis(100));
                }
            }
        }
    }

    #[allow(dead_code)]
    /// Pairs with device (QR flow — no connect).
    pub fn execute_pair_only(device: &DeviceInfo, password: &str) -> Result<DeviceInfo, String> {
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

        Ok(device.clone())
    }

    #[allow(dead_code)]
    pub fn execute_pair_and_connect(
        device: &DeviceInfo,
        password: &str,
    ) -> Result<DeviceInfo, String> {
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
