# Kanlo server on Debian 13

This archive includes the server and embedded web app. Native TV, kiosk setup
and flash images are not part of this development prerelease.

Choose `amd64` for x86-64, `arm64` for a 64-bit ARM OS or `armhf` for an ARMv7
hard-float OS. Match the operating system architecture. Physical Pi validation
remains outstanding. Verify downloads with `sha256sum -c SHA256SUMS`, extract,
then run as a regular user:

```sh
mkdir -p "$HOME/.local/share/kanlo"
KANLO_DATA_DIR="$HOME/.local/share/kanlo" ./bin/kanlo-server
```

Open `http://127.0.0.1:8080`. No display, Node, Docker or Internet connection is
required at runtime. Stop with Ctrl+C. `--check` verifies database integrity.

## Optional system service

From the extracted archive:

```sh
sudo install -Dm755 bin/kanlo-server /usr/local/lib/kanlo/kanlo-server
sudo install -Dm644 systemd/kanlo-server.service /etc/systemd/system/kanlo-server.service
sudo install -Dm644 sysusers.d/kanlo.conf /usr/lib/sysusers.d/kanlo.conf
sudo systemd-sysusers /usr/lib/sysusers.d/kanlo.conf
sudo systemctl daemon-reload
sudo systemctl enable --now kanlo-server
```

This installs the server only and does not change your desktop or enable a
kiosk. The unit uses a non-login account and private `/var/lib/kanlo` data.
Use `systemctl status kanlo-server` and `journalctl -u kanlo-server` for diagnosis.
Uninstall by stopping/disabling the service and removing installed program/unit
files. Keep data separately; it is never deleted automatically.

## Access and updates

Loopback is the default. LAN requires explicit allowed hosts. External access
requires a separate protected listener behind trusted HTTPS; never proxy the
LAN listener publicly. See packaged `docs/de/BETRIEB.md` and `docs/en/README.md`.
Remote-device offline PWA requires trusted HTTPS; ordinary LAN HTTP is online only.

Before a manual update, download a data backup, stop the service and retain the
previous archive. Install the new binary and restart. Older binaries refuse
newer database schemas. A data rollback requires the corresponding old binary
and a prior backup; automatic updates/rollback are not implemented yet.

For `Exec format error`, check OS/archive architecture. HDMI/display problems
must not block the server: this release contains no native TV process.
