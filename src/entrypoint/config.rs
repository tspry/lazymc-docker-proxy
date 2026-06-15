use serde::{Deserialize, Serialize};
use std::env::var;
use std::fs::File;
use std::io::Write;
use std::net::ToSocketAddrs;
use std::path::Path;
use std::process::{exit, Command};
use version_compare::Version;

use crate::{health, machine::MachineConfig};

const DEFAULT_PORT: i32 = 25565;

/// Sanitize a Minecraft MOTD string sourced from an environment variable.
///
/// YAML double-quoted strings interpret `\n` as a real newline character.
/// Minecraft's server.properties and lazymc's TOML config both expect the
/// two-character literal `\n`, not a real newline. A raw newline in
/// server.properties corrupts the Java Properties format and prevents Paper
/// from starting. This function converts real newlines back to `\n` and
/// strips other control characters that would produce invalid TOML or
/// break the Minecraft server-list-ping MOTD packet.
fn sanitize_motd(s: String) -> String {
    s.chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .collect::<String>()
        .replace('\n', "\\n")
}

/// lazymc dropped support for Minecraft servers older than 1.20.3
fn is_legacy(version: Option<&str>) -> bool {
    match version {
        None => false,
        Some(v) => Version::from(v) < Version::from("1.20.3"),
    }
}

#[derive(Serialize, Deserialize)]
struct ServerSection {
    address: Option<String>,
    block_banned_ips: Option<bool>,
    command: Option<String>,
    directory: Option<String>,
    drop_banned_ips: Option<bool>,
    forge: Option<bool>,
    freeze_process: Option<bool>,
    probe_on_start: Option<bool>,
    send_proxy_v2: Option<bool>,
    start_timeout: Option<i32>,
    stop_timeout: Option<i32>,
    wake_on_crash: Option<bool>,
    wake_on_start: Option<bool>,
    wake_whitelist: Option<bool>,
}

#[derive(Serialize, Deserialize)]
struct PublicSection {
    address: Option<String>,
    version: Option<String>,
    protocol: Option<i32>,
}

#[derive(Serialize, Deserialize)]
struct TimeSection {
    minimum_online_time: Option<i32>,
    sleep_after: Option<i32>,
}

#[derive(Serialize, Deserialize)]
struct JoinSection {
    methods: Option<Vec<String>>,
    kick: JoinKickSection,
    hold: JoinHoldSection,
    forward: JoinForwardSection,
    lobby: JoinLobbySection,
}

#[derive(Serialize, Deserialize)]
struct JoinKickSection {
    starting: Option<String>,
    stopping: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct JoinHoldSection {
    timeout: Option<i32>,
}

#[derive(Serialize, Deserialize)]
struct JoinForwardSection {
    address: Option<String>,
    send_proxy_v2: Option<bool>,
}

#[derive(Serialize, Deserialize)]
struct JoinLobbySection {
    timeout: Option<i32>,
    message: Option<String>,
    ready_sound: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct MotdSection {
    sleeping: Option<String>,
    starting: Option<String>,
    stopping: Option<String>,
    from_server: Option<bool>,
}

#[derive(Serialize, Deserialize)]
struct LockoutSection {
    enabled: Option<bool>,
    message: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct AdvancedSection {
    rewrite_server_properties: Option<bool>,
}

#[derive(Serialize, Deserialize)]
struct ConfigSection {
    version: Option<String>,
}

#[derive(Serialize)]
pub struct Config {
    advanced: AdvancedSection,
    config: ConfigSection,
    join: JoinSection,
    lockout: LockoutSection,
    motd: MotdSection,
    public: PublicSection,
    server: ServerSection,
    time: TimeSection,
    #[serde(skip)]
    start_command: String,
    #[serde(skip)]
    config_file: String,
    #[serde(skip)]
    group: String,
    #[serde(skip)]
    pub machine: MachineConfig,
}

impl Config {
    /// Build the lazymc start command
    pub fn start_command(&self) -> Command {
        let mut command = Command::new(self.start_command.clone());
        command.arg("start");
        command.arg("--config");
        command.arg(self.config_file.clone());
        command
    }

    pub fn group(&self) -> &str {
        &self.group
    }

    fn as_toml_string(&self) -> String {
        toml::to_string(self).unwrap_or_else(|e| {
            error!(target: "lazymc-docker-proxy::entrypoint::config", "Failed to serialize config to TOML: {}", e);
            health::unhealthy();
            exit(1);
        })
    }

    fn create_file(&self) {
        let toml = self.as_toml_string();
        let file_name = format!("lazymc.{}.toml", self.group);
        let path = Path::new(&file_name);
        let mut file = File::create(path).unwrap();
        file.write_all(toml.as_ref()).unwrap();
        debug!(target: "lazymc-docker-proxy::entrypoint::config", "Generated config `{}`:\n\n{}", path.display(), toml);
    }

    /// Build configuration from environment variables
    pub fn from_env() -> Self {
        let server_address = var("SERVER_ADDRESS").unwrap_or_else(|_| {
            error!(target: "lazymc-docker-proxy::entrypoint::config", "SERVER_ADDRESS is not set");
            health::unhealthy();
            exit(1);
        });

        let group = var("LAZYMC_GROUP").unwrap_or_else(|_| "mc".to_string());

        let version = var("PUBLIC_VERSION").ok();
        let legacy = is_legacy(version.as_deref());

        // Resolve the server address to an IP (lazymc requires a stable IP)
        let resolved_address = server_address
            .to_socket_addrs()
            .ok()
            .and_then(|mut addrs| addrs.find(|a| a.is_ipv4()))
            .map(|a| a.to_string())
            .unwrap_or_else(|| {
                warn!(target: "lazymc-docker-proxy::entrypoint::config",
                    "Could not resolve IP for SERVER_ADDRESS '{}'. Using value as-is.", server_address);
                server_address.clone()
            });

        let config_version = if legacy {
            var("LAZYMC_LEGACY_VERSION").unwrap_or_else(|err| {
                error!(target: "lazymc-docker-proxy::entrypoint::config", "LAZYMC_LEGACY_VERSION not set: {}", err);
                health::unhealthy();
                exit(1);
            })
        } else {
            var("LAZYMC_VERSION").unwrap_or_else(|err| {
                error!(target: "lazymc-docker-proxy::entrypoint::config", "LAZYMC_VERSION not set: {}", err);
                health::unhealthy();
                exit(1);
            })
        };

        let config = Config {
            server: ServerSection {
                address: Some(resolved_address),
                command: Some(format!("lazymc-docker-proxy --command --group {}", group)),
                directory: Some(
                    var("SERVER_DIRECTORY").unwrap_or_else(|_| "/server".to_string()),
                ),
                freeze_process: Some(false),
                wake_on_start: Some(true),
                wake_on_crash: Some(true),
                forge: var("SERVER_FORGE").ok().map(|v| v == "true"),
                probe_on_start: var("SERVER_PROBE_ON_START").ok().map(|v| v == "true"),
                block_banned_ips: var("SERVER_BLOCK_BANNED_IPS").ok().map(|v| v == "true"),
                drop_banned_ips: var("SERVER_DROP_BANNED_IPS").ok().map(|v| v == "true"),
                send_proxy_v2: var("SERVER_SEND_PROXY_V2").ok().map(|v| v == "true"),
                wake_whitelist: var("SERVER_WAKE_WHITELIST").ok().map(|v| v == "true"),
                start_timeout: var("SERVER_START_TIMEOUT").ok().and_then(|v| v.parse().ok()),
                stop_timeout: var("SERVER_STOP_TIMEOUT").ok().and_then(|v| v.parse().ok()),
            },
            public: PublicSection {
                address: Some(format!(
                    "0.0.0.0:{}",
                    var("LAZYMC_PORT").unwrap_or_else(|_| DEFAULT_PORT.to_string())
                )),
                version,
                protocol: var("PUBLIC_PROTOCOL").ok().and_then(|v| v.parse().ok()),
            },
            time: TimeSection {
                sleep_after: var("TIME_SLEEP_AFTER").ok().and_then(|v| v.parse().ok()),
                minimum_online_time: var("TIME_MINIMUM_ONLINE_TIME")
                    .ok()
                    .and_then(|v| v.parse().ok()),
            },
            join: JoinSection {
                methods: var("LAZYMC_JOIN_METHODS").ok().map(|v| {
                    v.split(',').map(|s| s.trim().to_string()).collect()
                }),
                kick: JoinKickSection {
                    starting: var("LAZYMC_JOIN_KICK_STARTING").ok(),
                    stopping: var("LAZYMC_JOIN_KICK_STOPPING").ok(),
                },
                hold: JoinHoldSection {
                    timeout: var("LAZYMC_JOIN_HOLD_TIMEOUT")
                        .ok()
                        .and_then(|v| v.parse().ok()),
                },
                forward: JoinForwardSection {
                    address: var("LAZYMC_JOIN_FORWARD_ADDRESS").ok(),
                    send_proxy_v2: var("LAZYMC_JOIN_FORWARD_SEND_PROXY_V2")
                        .ok()
                        .map(|v| v == "true"),
                },
                lobby: JoinLobbySection {
                    timeout: var("LAZYMC_JOIN_LOBBY_TIMEOUT")
                        .ok()
                        .and_then(|v| v.parse().ok()),
                    message: var("LAZYMC_JOIN_LOBBY_MESSAGE").ok(),
                    ready_sound: var("LAZYMC_JOIN_LOBBY_READY_SOUND").ok(),
                },
            },
            motd: MotdSection {
                sleeping: var("MOTD_SLEEPING").ok().map(sanitize_motd),
                starting: var("MOTD_STARTING").ok().map(sanitize_motd),
                stopping: var("MOTD_STOPPING").ok().map(sanitize_motd),
                from_server: var("MOTD_FROM_SERVER").ok().map(|v| v == "true"),
            },
            lockout: LockoutSection {
                enabled: var("LAZYMC_LOCKOUT_ENABLED").ok().map(|v| v == "true"),
                message: var("LAZYMC_LOCKOUT_MESSAGE").ok(),
            },
            advanced: AdvancedSection {
                rewrite_server_properties: Some(false),
            },
            config: ConfigSection {
                version: Some(config_version),
            },
            start_command: if legacy {
                "lazymc-legacy".to_string()
            } else {
                "lazymc".to_string()
            },
            config_file: format!("lazymc.{}.toml", group),
            group,
            machine: MachineConfig::from_env(),
        };

        config.create_file();
        config
    }
}
