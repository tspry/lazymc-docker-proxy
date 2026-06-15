# Setup Guide

## Hardware

- Raspberry Pi (any model with GPIO header)
- Single-channel relay module
- PC with Minecraft server, SSH enabled

### Relay Wiring

```
Pi GPIO pin 18 (BCM) ──► Relay IN / signal
Pi GND               ──► Relay GND
Pi 5V                ──► Relay VCC

Relay COM ──┐
            ├── across the 2-pin power-button header on the PC motherboard
Relay NO  ──┘
```

When the Pi holds pin 18 HIGH for 500 ms, the relay closes and simulates a power button press. Change `GPIO_PIN` and `GPIO_PULSE_MS` in the Pi compose to match your wiring.

---

## SSH Access (Pi → PC)

On the Pi:

```bash
ssh-keygen -t ed25519 -f ~/.ssh/mc_key -N ""
ssh-copy-id -i ~/.ssh/mc_key.pub youruser@192.168.1.100
```

On the PC — allow passwordless shutdown and Docker:

```bash
# passwordless shutdown
echo "youruser ALL=(ALL) NOPASSWD: /sbin/shutdown" | sudo tee /etc/sudoers.d/mc-shutdown

# passwordless docker
sudo usermod -aG docker youruser
# log out and back in for the group to take effect
```

Verify from the Pi:

```bash
ssh -i ~/.ssh/mc_key youruser@192.168.1.100 "docker ps"
```

---

## PC — Minecraft Server

Create a `docker-compose.yml` on the PC. No lazymc, no proxy — the Pi controls the lifecycle.

```yaml
services:
  mc:
    image: itzg/minecraft-server:latest
    restart: unless-stopped      # auto-starts when PC boots
    ports:
      - "25565:25565"
    tty: true
    stdin_open: true
    environment:
      EULA: "TRUE"
      TYPE: "PAPER"
      INIT_MEMORY: "1024M"
      MAX_MEMORY: "1536M"
      ONLINE_MODE: "false"
      USE_AIKAR_FLAGS: "true"
      TZ: "Asia/Kathmandu"
      DIFFICULTY: "3"
      LEVEL: "paraverse"
      SPAWN_PROTECTION: "16"
      MOTD: "      Paradox Time Server\\n        Have Fun!!"
      ICON: "https://i.postimg.cc/y7nFdLR8/pdt-logo-nobg-shadow.png"
      OVERRIDE_ICON: "true"
      MODRINTH_PROJECTS: |
        authmerereloaded
        viaversion
        viabackwards
        chunky
        skinsrestorer
        tablistping
      VERSION_FROM_MODRINTH_PROJECTS: true
    volumes:
      - "./data:/data"
    healthcheck:
      disable: true
```

Pull the image and create the container once, then leave it stopped — the Pi will start it:

```bash
docker compose up -d
docker compose down
```

---

## Pi — Proxy

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
      RUST_LOG: "info"

      SERVER_ADDRESS: "192.168.1.100:25565"
      MACHINE_HOST: "192.168.1.100"
      MACHINE_SSH_USER: "youruser"
      MACHINE_SSH_KEY_PATH: "/keys/id_rsa"

      GPIO_PIN: "18"
      GPIO_PULSE_MS: "500"

      # uncomment if your NIC/BIOS supports Wake-on-LAN
      # MACHINE_WOL_MAC: "AA:BB:CC:DD:EE:FF"

      MC_MANAGEMENT: "docker"
      MC_DOCKER_CONTAINER: "mc"

      LAZYMC_GROUP: "mc"
      TIME_SLEEP_AFTER: "300"
      TIME_MINIMUM_ONLINE_TIME: "300"
      SERVER_SEND_PROXY_V2: "true"
```

Port-forward port `25565` on your router to the Pi's LAN IP for external players.

---

## Real Player IPs (Proxy Protocol)

By default PaperMC sees all connections coming from the Pi's IP. To pass real player IPs through:

**Pi compose** (already set above):
```yaml
SERVER_SEND_PROXY_V2: "true"
```

**On the PC**, edit `./data/config/paper-global.yml`:
```yaml
proxies:
  proxy-protocol: true
```

Restart the MC container. PaperMC will now read the HAProxy v2 header lazymc attaches to every forwarded connection and use the real player IP for logging, bans, and whitelists.

> Once `proxy-protocol: true` is set, direct connections to the PC's port 25565 (bypassing the Pi) will be rejected. Players must go through the Pi.
