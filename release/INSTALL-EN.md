# Kanlo server on Debian 13

This archive includes the server and embedded web app for an existing desktop
or headless system. The separate [appliance image](IMAGES-EN.md) includes the
native kiosk display and local Wi-Fi setup.

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
./install.sh --check
sudo ./install.sh
```

The installer checks the operating system, architecture and binary checksum.
It installs a server service with a non-login account and private
`/var/lib/kanlo` data.
Use `systemctl status kanlo-server` and `journalctl -u kanlo-server` for diagnosis.
An upgrade retains the previous binary as `kanlo-server.previous`. Run
`sudo ./install.sh --uninstall` from the package to remove the installed
services. Data and configuration remain on the system.

### Optional native display

On a dedicated Linux system with a free local console, the server installer
can also install the display.

```sh
sudo apt install cage seatd python3-pygame fonts-dejavu-core
sudo ./install.sh --kiosk
```

The kiosk uses tty1 and requires an inactive desktop display manager.
Use the appliance image for on-screen Wi-Fi setup.

## Access and updates

Loopback is the default. LAN requires explicit allowed hosts. External access
requires a separate protected listener behind trusted HTTPS; never proxy the
LAN listener publicly. See packaged `docs/de/BETRIEB.md` and `docs/en/README.md`.
Remote-device offline PWA requires trusted HTTPS; ordinary LAN HTTP is online only.

Before a manual update, download a data backup, stop the service and retain the
previous archive. Install the new binary and restart. Older binaries refuse
newer database schemas. A data rollback requires the corresponding old binary
and a prior backup; automatic updates/rollback are not implemented yet.

For `Exec format error`, compare the archive with `dpkg --print-architecture`.
See the [appliance guide](IMAGES-EN.md) for Pi 3 B+ HDMI and composite troubleshooting.
