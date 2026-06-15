# Configuration Reference

All configuration is via environment variables on the Pi's proxy container.

## Required

| Variable | Description |
|---|---|
| `SERVER_ADDRESS` | LAN IP and port of the PC's Minecraft server, e.g. `192.168.1.100:25565` |
| `MACHINE_HOST` | PC IP or hostname for SSH. Inferred from `SERVER_ADDRESS` if omitted. |

## Power Control

| Variable | Default | Description |
|---|---|---|
| `GPIO_PIN` | *(disabled)* | BCM pin number wired to the relay signal pin |
| `GPIO_PULSE_MS` | `500` | How long to hold the relay closed (milliseconds) |
| `MACHINE_WOL_MAC` | *(disabled)* | PC MAC address for Wake-on-LAN, e.g. `AA:BB:CC:DD:EE:FF` |
| `MACHINE_WOL_BROADCAST` | `255.255.255.255` | Broadcast address for WoL packets |

GPIO and WoL can be used together — both fire on every power-on event.

## SSH

| Variable | Default | Description |
|---|---|---|
| `MACHINE_SSH_USER` | `ubuntu` | SSH username on the PC |
| `MACHINE_SSH_KEY_PATH` | *(none)* | Path to the private key file inside the container (mount via volume) |
| `SSH_RETRY_MAX` | `30` | Max SSH attempts while waiting for the PC to finish booting |
| `SSH_RETRY_INTERVAL_S` | `5` | Seconds between retry attempts (30 × 5 s = 2.5 min max wait) |

## Minecraft Management

| Variable | Default | Description |
|---|---|---|
| `MC_MANAGEMENT` | `boot` | How Minecraft is managed on the PC: `docker`, `systemd`, or `boot` |
| `MC_DOCKER_CONTAINER` | `minecraft` | Container name (when `MC_MANAGEMENT=docker`) |
| `MC_SYSTEMD_SERVICE` | `minecraft.service` | Service name (when `MC_MANAGEMENT=systemd`) |

### MC_MANAGEMENT modes

| Mode | On power-on | On idle |
|---|---|---|
| `docker` | SSH `docker start <container>` (waits for PC to boot first) | SSH `docker stop <container>` then `shutdown` |
| `systemd` | SSH `systemctl start <service>` (waits for PC to boot first) | SSH `systemctl stop <service>` then `shutdown` |
| `boot` | Nothing — MC starts automatically when PC boots | SSH `shutdown` (OS stops MC gracefully) |

Use `docker` or `systemd` for survival servers where you want an explicit clean save before shutdown. Use `boot` if the container has `restart: unless-stopped` and you trust Docker's shutdown handling.

## Proxy / lazymc Settings

| Variable | Default | Description |
|---|---|---|
| `LAZYMC_GROUP` | `mc` | Internal label used in logs and the generated config file name |
| `LAZYMC_PORT` | `25565` | Port the proxy listens on |
| `SERVER_SEND_PROXY_V2` | *(none)* | `true` to prepend HAProxy v2 headers (required when Paper has `proxy-protocol: true`) |
| `TIME_SLEEP_AFTER` | *(lazymc default)* | Seconds of no players before sleeping |
| `TIME_MINIMUM_ONLINE_TIME` | *(lazymc default)* | Minimum seconds to stay online after the server starts |
| `PUBLIC_VERSION` | *(none)* | Minecraft version string, e.g. `1.21.4` |
| `PUBLIC_PROTOCOL` | *(none)* | Protocol number — see [minecraft.wiki/w/Protocol_version](https://minecraft.wiki/w/Protocol_version) |
| `SERVER_FORGE` | *(none)* | `true` for Forge servers |
| `SERVER_DIRECTORY` | `/server` | Local path for ban/whitelist files |
| `MOTD_SLEEPING` | *(none)* | Server browser MOTD while sleeping |
| `MOTD_STARTING` | *(none)* | Server browser MOTD while starting |
| `MOTD_STOPPING` | *(none)* | Server browser MOTD while stopping |
| `MOTD_FROM_SERVER` | *(none)* | `true` to use the actual server MOTD once known |
| `LAZYMC_JOIN_METHODS` | *(none)* | Comma-separated: `hold`, `kick`, `lobby`, `forward` |
| `LAZYMC_JOIN_HOLD_TIMEOUT` | *(none)* | Seconds to hold a connecting client while server starts (max 30) |
| `LAZYMC_JOIN_KICK_STARTING` | *(none)* | Message shown when kicking during startup |
| `LAZYMC_JOIN_KICK_STOPPING` | *(none)* | Message shown when kicking during shutdown |
| `LAZYMC_JOIN_LOBBY_TIMEOUT` | *(none)* | Max seconds in lobby while server starts |
| `LAZYMC_JOIN_LOBBY_MESSAGE` | *(none)* | Banner shown in the lobby |
| `LAZYMC_JOIN_LOBBY_READY_SOUND` | *(none)* | Sound played when server is ready |
| `LAZYMC_JOIN_FORWARD_ADDRESS` | *(none)* | Address to forward connections to (alternative to proxying) |
| `LAZYMC_JOIN_FORWARD_SEND_PROXY_V2` | *(none)* | `true` to add HAProxy v2 header to forwarded connections |
| `LAZYMC_LOCKOUT_ENABLED` | *(none)* | `true` to block all connections |
| `LAZYMC_LOCKOUT_MESSAGE` | *(none)* | Message shown when locked out |
| `RUST_LOG` | `info` | Log verbosity: `trace`, `debug`, `info`, `warn`, `error` |
