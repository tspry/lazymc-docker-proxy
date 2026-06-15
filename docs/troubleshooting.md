# Troubleshooting

Set `RUST_LOG: "debug"` in the Pi compose to get verbose output for all issues.

## PC does not power on

- Check the relay wiring — the relay NO (Normally Open) contacts should be across the two power-button header pins on the motherboard (the same connector your case's power button plugs into)
- Look for `GPIO: pulsing pin 18 for 500ms` in the logs — if this line appears, the software side is working and the issue is hardware
- Try bridging the power-button header pins manually with a jumper wire to confirm the PC boots that way
- If GPIO is uncertain, add `MACHINE_WOL_MAC` as a fallback (requires WoL enabled in BIOS and the NIC set to wake on magic packet)

## SSH times out / MC never starts

- Check the Pi can reach the PC manually:
  ```bash
  ssh -i ~/.ssh/mc_key youruser@192.168.1.100 "echo ok"
  ```
- If the PC takes more than 2.5 minutes to boot, increase `SSH_RETRY_MAX` (default 30 × 5 s intervals)
- Confirm SSH is set to start on boot on the PC:
  ```bash
  sudo systemctl enable ssh
  ```
- Make sure the PC's firewall allows SSH from the Pi's IP

## Players see the Pi's IP instead of their real IP

Both sides of the proxy protocol must be configured together:

- Pi compose: `SERVER_SEND_PROXY_V2: "true"`
- PC `./data/config/paper-global.yml`: `proxies: proxy-protocol: true`

One without the other either shows the wrong IP or breaks all connections.

## Direct connections to the PC are rejected

This is expected behaviour once `proxy-protocol: true` is set in `paper-global.yml`. PaperMC requires the HAProxy header on every connection — connections that don't come through the Pi proxy are missing the header and get dropped. Players must connect to the Pi's IP.

## Server does not go to sleep

- Confirm `TIME_SLEEP_AFTER` is set in seconds, not minutes (e.g. `300` = 5 minutes)
- Check the logs for lazymc's idle detection messages
- Make sure no background process is keeping the server "active" (bots, plugins that simulate activity)

## Paper fails to start / server.properties parse error

This is caused by a real newline character inside `server.properties`. It happens when the `MOTD` environment variable in the PC's docker-compose uses a YAML double-quoted string with `\n`:

```yaml
# WRONG — YAML interprets \n as a real newline, which corrupts server.properties
MOTD: "Paradox Server\n Have Fun"

# CORRECT — double backslash produces the literal two-character \n that Java expects
MOTD: "Paradox Server\\n Have Fun"
```

Java's Properties format uses `\n` as a two-character escape for line breaks. A real newline in the file breaks the parser and Paper refuses to start.

The same rule applies to `MOTD_SLEEPING`, `MOTD_STARTING`, and `MOTD_STOPPING` on the Pi side — use `\\n` in YAML double-quoted strings for line breaks. The proxy sanitizes these values automatically, but it is better to write them correctly in the first place.

## World data lost / server did not save before shutdown

- Use `MC_MANAGEMENT=docker` or `MC_MANAGEMENT=systemd` instead of `boot` — these issue an explicit `docker stop` / `systemctl stop` before SSH shutdown, giving PaperMC time to save
- Avoid `MC_MANAGEMENT=boot` for survival servers with important player data

## Container starts but MC is not reachable from the Pi

- Confirm the MC container exposes port 25565 on the PC's LAN interface (not just `127.0.0.1`)
- Check `SERVER_ADDRESS` matches the PC's actual LAN IP
- Temporarily disable any firewall on the PC and test with `nc -zv 192.168.1.100 25565` from the Pi
