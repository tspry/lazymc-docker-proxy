use std::{
    env::var,
    net::UdpSocket,
    process::Command,
    thread,
    time::Duration,
};

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
use rppal::gpio::Gpio;

#[derive(Debug, Clone, PartialEq)]
pub enum McManagement {
    Docker,
    Systemd,
    Boot,
}

impl McManagement {
    fn from_env_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "docker" => McManagement::Docker,
            "systemd" => McManagement::Systemd,
            _ => McManagement::Boot,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MachineConfig {
    pub host: String,
    pub ssh_user: String,
    pub ssh_key_path: Option<String>,
    pub wol_mac: Option<String>,
    pub wol_broadcast: String,
    pub gpio_pin: Option<u8>,
    pub gpio_pulse_ms: u64,
    pub mc_management: McManagement,
    pub mc_docker_container: String,
    pub mc_systemd_service: String,
    pub ssh_retry_max: u32,
    pub ssh_retry_interval_s: u64,
}

impl MachineConfig {
    pub fn from_env() -> Self {
        let host = var("MACHINE_HOST")
            .or_else(|_| {
                var("SERVER_ADDRESS").map(|addr| {
                    addr.split(':').next().unwrap_or("").to_string()
                })
            })
            .expect("MACHINE_HOST or SERVER_ADDRESS must be set");

        MachineConfig {
            host,
            ssh_user: var("MACHINE_SSH_USER").unwrap_or_else(|_| "ubuntu".to_string()),
            ssh_key_path: var("MACHINE_SSH_KEY_PATH").ok(),
            wol_mac: var("MACHINE_WOL_MAC").ok(),
            wol_broadcast: var("MACHINE_WOL_BROADCAST")
                .unwrap_or_else(|_| "255.255.255.255".to_string()),
            gpio_pin: var("GPIO_PIN").ok().and_then(|s| s.parse().ok()),
            gpio_pulse_ms: var("GPIO_PULSE_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(500),
            mc_management: McManagement::from_env_str(
                &var("MC_MANAGEMENT").unwrap_or_else(|_| "boot".to_string()),
            ),
            mc_docker_container: var("MC_DOCKER_CONTAINER")
                .unwrap_or_else(|_| "minecraft".to_string()),
            mc_systemd_service: var("MC_SYSTEMD_SERVICE")
                .unwrap_or_else(|_| "minecraft.service".to_string()),
            ssh_retry_max: var("SSH_RETRY_MAX")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(30),
            ssh_retry_interval_s: var("SSH_RETRY_INTERVAL_S")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
        }
    }
}

/// Power on the remote PC via GPIO relay pulse and/or Wake-on-LAN
pub fn power_on(config: &MachineConfig) {
    info!(target: "lazymc-docker-proxy::machine", "Powering on PC...");

    if let Some(pin) = config.gpio_pin {
        gpio_pulse(pin, config.gpio_pulse_ms);
    }

    if let Some(mac) = &config.wol_mac {
        send_wol(mac, &config.wol_broadcast);
    }
}

/// Start the Minecraft server on the remote PC
pub fn start_server(config: &MachineConfig) {
    let interval = Duration::from_secs(config.ssh_retry_interval_s);
    match &config.mc_management {
        McManagement::Docker => {
            info!(target: "lazymc-docker-proxy::machine", "Waiting for PC to boot, then starting Minecraft Docker container...");
            let cmd = format!("docker start {}", config.mc_docker_container);
            ssh_exec_with_retry(config, &cmd, config.ssh_retry_max, interval);
        }
        McManagement::Systemd => {
            info!(target: "lazymc-docker-proxy::machine", "Waiting for PC to boot, then starting Minecraft systemd service...");
            let cmd = format!("sudo systemctl start {}", config.mc_systemd_service);
            ssh_exec_with_retry(config, &cmd, config.ssh_retry_max, interval);
        }
        McManagement::Boot => {
            info!(target: "lazymc-docker-proxy::machine", "Boot mode: Minecraft starts automatically on PC boot.");
        }
    }
}

/// Stop the Minecraft server on the remote PC
pub fn stop_server(config: &MachineConfig) {
    match &config.mc_management {
        McManagement::Docker => {
            info!(target: "lazymc-docker-proxy::machine", "Stopping Minecraft Docker container on PC...");
            let cmd = format!("docker stop {}", config.mc_docker_container);
            ssh_exec(config, &cmd);
        }
        McManagement::Systemd => {
            info!(target: "lazymc-docker-proxy::machine", "Stopping Minecraft systemd service on PC...");
            let cmd = format!("sudo systemctl stop {}", config.mc_systemd_service);
            ssh_exec(config, &cmd);
        }
        McManagement::Boot => {
            info!(target: "lazymc-docker-proxy::machine", "Boot mode: OS shutdown will stop Minecraft gracefully.");
        }
    }
}

/// Shut down the remote PC via SSH
pub fn shutdown_pc(config: &MachineConfig) {
    info!(target: "lazymc-docker-proxy::machine", "Shutting down PC...");
    ssh_exec(config, "sudo shutdown -h now");
}

/// Pulse a GPIO pin HIGH for the given duration to trigger a relay (ARM/RPi only)
fn gpio_pulse(pin: u8, duration_ms: u64) {
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    {
        info!(target: "lazymc-docker-proxy::machine", "GPIO: pulsing pin {} for {}ms...", pin, duration_ms);
        match Gpio::new().and_then(|gpio| gpio.get(pin)) {
            Ok(handle) => {
                let mut output = handle.into_output();
                output.set_high();
                thread::sleep(Duration::from_millis(duration_ms));
                output.set_low();
                info!(target: "lazymc-docker-proxy::machine", "GPIO: pulse on pin {} complete.", pin);
            }
            Err(e) => {
                error!(target: "lazymc-docker-proxy::machine", "GPIO error on pin {}: {}", pin, e);
            }
        }
    }
    #[cfg(not(any(target_arch = "arm", target_arch = "aarch64")))]
    {
        info!(target: "lazymc-docker-proxy::machine", "GPIO stub (non-ARM build): would pulse pin {} for {}ms", pin, duration_ms);
    }
}

/// Send a Wake-on-LAN magic packet (6×0xFF + 16× MAC address) via UDP broadcast
fn send_wol(mac: &str, broadcast: &str) {
    let mac_bytes: Vec<u8> = mac
        .split(|c| c == ':' || c == '-')
        .filter_map(|s| u8::from_str_radix(s, 16).ok())
        .collect();

    if mac_bytes.len() != 6 {
        error!(target: "lazymc-docker-proxy::machine", "WoL: invalid MAC address '{}' (expected 6 hex octets)", mac);
        return;
    }

    let mut packet = vec![0xFF_u8; 6];
    for _ in 0..16 {
        packet.extend_from_slice(&mac_bytes);
    }

    match UdpSocket::bind("0.0.0.0:0") {
        Ok(socket) => {
            if socket.set_broadcast(true).is_err() {
                error!(target: "lazymc-docker-proxy::machine", "WoL: failed to enable broadcast on socket");
                return;
            }
            let target = format!("{}:9", broadcast);
            match socket.send_to(&packet, &target) {
                Ok(_) => info!(target: "lazymc-docker-proxy::machine", "WoL: magic packet sent to {} via {}", mac, target),
                Err(e) => error!(target: "lazymc-docker-proxy::machine", "WoL: failed to send magic packet: {}", e),
            }
        }
        Err(e) => error!(target: "lazymc-docker-proxy::machine", "WoL: failed to bind UDP socket: {}", e),
    }
}

/// Run a command on the remote PC via SSH, returns true on success
pub fn ssh_exec(config: &MachineConfig, cmd: &str) -> bool {
    debug!(target: "lazymc-docker-proxy::machine", "SSH [{}@{}]: {}", config.ssh_user, config.host, cmd);

    let mut ssh = Command::new("ssh");
    ssh.arg("-o").arg("StrictHostKeyChecking=no");
    ssh.arg("-o").arg("ConnectTimeout=10");
    ssh.arg("-o").arg("BatchMode=yes");

    if let Some(key_path) = &config.ssh_key_path {
        ssh.arg("-i").arg(key_path);
    }

    ssh.arg(format!("{}@{}", config.ssh_user, config.host));
    ssh.arg(cmd);

    match ssh.output() {
        Ok(output) if output.status.success() => {
            debug!(target: "lazymc-docker-proxy::machine", "SSH: command succeeded");
            true
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            debug!(target: "lazymc-docker-proxy::machine", "SSH: command failed (exit {}): {}", output.status, stderr.trim());
            false
        }
        Err(e) => {
            error!(target: "lazymc-docker-proxy::machine", "SSH: failed to spawn ssh process: {}", e);
            false
        }
    }
}

/// Retry `ssh_exec` until success or `max_retries` is exhausted
fn ssh_exec_with_retry(config: &MachineConfig, cmd: &str, max_retries: u32, interval: Duration) {
    for attempt in 1..=max_retries {
        debug!(target: "lazymc-docker-proxy::machine", "SSH attempt {}/{}: {}", attempt, max_retries, cmd);
        if ssh_exec(config, cmd) {
            return;
        }
        if attempt < max_retries {
            debug!(target: "lazymc-docker-proxy::machine", "PC not reachable yet, retrying in {:?}...", interval);
            thread::sleep(interval);
        }
    }
    error!(target: "lazymc-docker-proxy::machine", "SSH command failed after {} attempts: {}", max_retries, cmd);
}
