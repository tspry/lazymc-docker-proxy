# lazymc-docker-proxy

A fork of [lazymc-docker-proxy](https://github.com/joesturge/lazymc-docker-proxy) adapted for **Raspberry Pi + physical PC** setups.

The Pi runs as a Minecraft proxy. When a player connects, it powers on the PC via a GPIO relay (and optionally Wake-on-LAN), waits for the server to come up, then proxies all traffic through. When the server is idle, it shuts Minecraft down and powers the PC off.

```
Player ──► Pi (proxy + GPIO) ──► PC (Minecraft server)
               │
               ├── GPIO relay ──► PC power button
               ├── Wake-on-LAN (optional)
               └── SSH ──► docker start/stop + shutdown
```

## Quick Start

**Pi** — `docker-compose.yml`:

```yaml
services:
  lazymc:
    image: ghcr.io/tspry/lazymc-docker-proxy:latest
    restart: unless-stopped
    devices:
      - /dev/gpiomem:/dev/gpiomem
    volumes:
      - ~/.ssh/mc_key:/keys/id_rsa:ro
    ports:
      - "25565:25565"
    environment:
      SERVER_ADDRESS: "192.168.1.100:25565"
      MACHINE_HOST: "192.168.1.100"
      MACHINE_SSH_USER: "youruser"
      MACHINE_SSH_KEY_PATH: "/keys/id_rsa"
      GPIO_PIN: "18"
      MC_MANAGEMENT: "docker"
      MC_DOCKER_CONTAINER: "mc"
      TIME_SLEEP_AFTER: "300"
      TIME_MINIMUM_ONLINE_TIME: "300"
      SERVER_SEND_PROXY_V2: "true"
```

**PC** — `docker-compose.yml`:

```yaml
services:
  mc:
    image: itzg/minecraft-server:latest
    restart: unless-stopped
    ports:
      - "25565:25565"
    environment:
      EULA: "TRUE"
      TYPE: "PAPER"
      # ... your MC settings
    volumes:
      - "./data:/data"
```

## Docs

- [Full Setup Guide](docs/setup.md) — hardware wiring, SSH keys, sudoers, proxy protocol
- [Configuration Reference](docs/configuration.md) — all environment variables
- [Troubleshooting](docs/troubleshooting.md)

## Credits

Based on [lazymc-docker-proxy](https://github.com/joesturge/lazymc-docker-proxy) by [@joesturge](https://github.com/joesturge).  
Uses [lazymc](https://github.com/timvisee/lazymc) by [@timvisee](https://github.com/timvisee).
