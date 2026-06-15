# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A fork of `lazymc-docker-proxy` adapted for **Raspberry Pi + physical PC** setups. Instead of controlling a local Docker container, it controls a remote PC via:

- **GPIO relay** → PC power button header (turns the PC on)
- **Wake-on-LAN** → UDP magic packet (optional additional power-on)
- **SSH** → starts/stops Minecraft and shuts the PC down when idle

[lazymc](https://github.com/timvisee/lazymc) handles the Minecraft protocol proxy — players connecting to the Pi see the sleeping MOTD, get held in a lobby while the PC boots, then are forwarded once the MC server is ready.

## Build & Development Commands

```bash
# Build binary
cargo build --release

# Cross-compile for Raspberry Pi (arm64)
cargo build --target aarch64-unknown-linux-musl --release

# Build Docker image
docker build .

# Run bats integration tests (requires Docker + bats)
bats ./tests/bats/<test-name>

# Debug logging
RUST_LOG=debug ./target/release/lazymc-docker-proxy
```

## Architecture

The binary has three operating modes, selected by CLI flags:

| Mode | Flag | Role |
|------|------|------|
| Entrypoint | *(none)* | Reads env vars, generates `lazymc.<group>.toml`, spawns `lazymc` |
| Command | `--command --group <name>` | Called by lazymc: powers on PC, starts MC; on SIGTERM stops MC and shuts down PC |
| Health check | `--health` | Reads `/app/health` (STARTING/HEALTHY/UNHEALTHY) and exits 0/1 |

### Source layout

- `src/main.rs` — CLI parsing, dispatches to one of three modes
- `src/machine.rs` — Core hardware control module:
  - `power_on()` — GPIO relay pulse + WoL UDP magic packet
  - `start_server()` — SSH to run `docker start`, `systemctl start`, or no-op (boot mode)
  - `stop_server()` — SSH to run `docker stop`, `systemctl stop`, or no-op (boot mode)
  - `shutdown_pc()` — SSH `sudo shutdown -h now`
  - `MachineConfig` — parsed from env vars
  - GPIO code is conditionally compiled: `#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]`
- `src/command/mod.rs` — `--command` mode: calls `machine::power_on → start_server`, waits for SIGTERM, then calls `stop_server → shutdown_pc`
- `src/entrypoint/mod.rs` — Spawns `lazymc` child, forwards its logs, handles unexpected shutdown states via `machine::stop_server + shutdown_pc`
- `src/entrypoint/config.rs` — Builds `Config` struct from env vars; serializes to `lazymc.<group>.toml` via serde/toml; selects `lazymc` or `lazymc-legacy` binary based on MC version (<1.20.3 uses legacy); holds `MachineConfig`
- `src/health.rs` — File-based health state machine at `/app/health`
- `src/logging.rs` — Initializes `pretty_env_logger`

### Key design details

- **GPIO conditional compile**: `rppal` is only included as a dependency when `target_arch` is `arm` or `aarch64`. On amd64 builds, `gpio_pulse()` is a no-op stub that logs a message. This lets CI/Docker amd64 builds succeed without RPi hardware.
- **SSH retry loop**: In `docker`/`systemd` management modes, `start_server()` retries SSH every `SSH_RETRY_INTERVAL_S` seconds (up to `SSH_RETRY_MAX` attempts) waiting for the PC to finish booting before starting Minecraft. lazymc then independently probes the MC port.
- **`wake_on_start=true`**: Always set in the generated lazymc config so lazymc invokes `--command` on startup (which powers on the PC).
- **`wake_on_crash=true`**: Always set so the PC is restarted if lazymc detects the MC server crashed.
- **Dual lazymc binary**: Dockerfile builds both `lazymc` (≥0.2.11) and `lazymc-legacy` (0.2.10). `config.rs` selects between them based on `PUBLIC_VERSION`.
- **Alpine base image**: The final Docker image uses Alpine (not scratch) so the `ssh` client binary is available at runtime for SSH control of the remote PC.

### MC_MANAGEMENT modes

| Value | Start behaviour | Stop behaviour |
|-------|----------------|----------------|
| `docker` | SSH → `docker start <MC_DOCKER_CONTAINER>` | SSH → `docker stop <MC_DOCKER_CONTAINER>` |
| `systemd` | SSH → `sudo systemctl start <MC_SYSTEMD_SERVICE>` | SSH → `sudo systemctl stop <MC_SYSTEMD_SERVICE>` |
| `boot` | no-op (MC auto-starts on boot) | no-op (OS shutdown handles it) |

### Required env vars at runtime

```
SERVER_ADDRESS=192.168.1.100:25565   # MC server on the PC
MACHINE_HOST=192.168.1.100           # PC SSH target (inferred from SERVER_ADDRESS if omitted)
```

All other variables are optional — see README.md for the full list.

## Commit Style

Use [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/). Update `CHANGELOG.md` under `[Unreleased]` for user-facing changes.
