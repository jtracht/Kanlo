# Kanlo appliance images

These images turn a **dedicated device** into a Kanlo appliance. Flashing erases
the entire chosen target drive. Use the standalone server package to integrate
Kanlo with an existing desktop instead. These are private development images.

| Image | Target |
|---|---|
| rpi-armhf | Raspberry Pi 3 B/B+, 4/400; ARMv7 32-bit, preferred on 1 GiB RAM |
| rpi-arm64 | Pi 3 and later, 4/5/400/500, Zero 2 W; 64-bit |
| generic-amd64 | x86-64 PC/mini-PC/VM with UEFI and Debian-supported hardware |
| generic-arm64 | ARM64 UEFI/SBSA system/VM; not arbitrary SBCs or Apple Silicon |

Original Pi 1/Zero are unsupported. Physical device/HDMI/Wi-Fi acceptance and
Secure Boot acceptance remain open. QEMU tests boot real kernels and systemd;
they do not certify physical hardware or performance.

Download the chosen `.img.xz`, its `.image.json`, and `SHA256SUMS`. Verify the
chosen file's SHA256. Flash with Raspberry Pi Imager's custom-image option or a
raw-image flasher onto an explicitly selected SD/USB/SSD of **at least 16 GB**.
All existing data on that target is lost. Build scripts never flash real drives.

Optionally copy `KANLO_CONFIG.example.json` to `KANLO_CONFIG.json` on the FAT
boot partition. Set a hostname, IANA timezone, `display` boolean, optional
`wifi` object (`ssid`, `password`, two-letter uppercase `country`) and optional
single `ssh_authorized_key`. Use only your own public key, never a private key.
Use `null` for unwanted Wi-Fi/SSH setup. Successful provisioning deletes the
real configuration file from the boot partition; the example contains no secrets.

Connect Ethernet or provision Wi-Fi and boot. Storage expands automatically.
The server starts offline, without login. Default hostnames are
`kanlo-<device-identity>`. Open the address shown on the attached display or on
the physical Ctrl+Alt+F2 setup console: `http://<hostname>.local:8080` or the
device's LAN IPv4 address. If mDNS is blocked, inspect your router's DHCP list.
Press **Ctrl+Alt+F2**, choose **1** to select a Wi-Fi network, set the country
if prompted, then enter your password in the network dialog. After closing it,
check the connection status and Kanlo address. **Ctrl+Alt+F1** returns to the board.

Connect HDMI before boot and select the matching monitor input. A Raspberry Pi
composite output requires a compatible connector and cable and may require
firmware configuration. On affected models, enabling composite disables HDMI.
See the [Raspberry Pi composite documentation](https://www.raspberrypi.com/documentation/computers/config_txt.html#composite-video-mode).
If the display stays on boot messages, open the setup console and select **3**
for connector and service diagnostics or **4** to restart the board display.

There is no shared password or open root shell. SSH stays off until you supply
your own public key. That key grants full device administration as
`kanlo-admin` with sudo. The physical-console owner can reconfigure network,
hostname, timezone and display settings locally. Do not expose LAN port 8080
to the internet. Configure external HTTPS/PWA through the separate protected
ingress described in the server operating guide.

The server and initial native read-only board viewer have separate services.
The viewer uses SDL/Wayland/Cage without Chromium or a full desktop. Headless
operation remains available. Display hotplug can start the viewer, and a
graphics startup failure falls back to a simple board on the local console. Full TV
overlays/Markdown/statistics/control remain under development.

SQLite is persistent in private `/var/lib/kanlo` storage. Volatile logs are
capped at 32 MiB; compressed RAM swap at half RAM or 512 MiB. There is no SD
swap, overclocking, disabled SQLite fsync, or volatile root overlay discarding
your data. Back up Kanlo data before upgrades/reflashing. Reflashing erases it;
an automatic A/B updater/rollback is not included. After optional SSH setup,
use normal Debian tools for OS security updates and `systemctl`/`journalctl`
to diagnose `kanlo-firstboot`, `kanlo-server` and `kanlo-tv`.

Every CI build verifies pinned official base-image hashes, installs all runtime
dependencies before shipping, records installed OS packages and the exact
Kanlo commit, and boots an isolated image copy twice. Tests cover provisioning,
disk expansion, DHCP/LAN HTTP access, bounded RAM-only swap, service restart,
private storage, persisted tickets after reboot, and SQLite integrity.
Test users/data/hooks never enter the shipped image.
Version tags publish only after all four image profiles and other quality gates
pass. Each image's JSON report and release checksums accompany the download.
ARM boot tests use a pinned upstream QEMU build with the proper BCM2835 watchdog
model, retaining the original Pi watchdog configuration. Reports record emulator
version, source commit and binary hash. The emulator fix does not enter the OS image.
Pi tests attach the isolated disk through emulated USB storage and use ordinary
USB interrupts because the emulator does not faithfully implement SD DMA/FIQ
paths. Direct ARM64 kernel boot decompresses the vendor gzip kernel as real Pi
firmware would. Shipping firmware/configuration retains the vendor defaults. These tests
do not certify real-device storage/network performance or HDMI/Wi-Fi hardware.
Both Pi profiles are tested on the Pi 3 model with the vendor 64-bit kernel;
the ARMHF profile executes its actual 32-bit userspace/server through the
kernel's compatibility support. Native 32-bit kernel boot remains a physical
hardware acceptance item. Reports identify both kernel and userspace architecture.
WPA-Supplicant and wireless regulatory data are preinstalled. Generic images
also include common Intel, Realtek, Broadcom, Qualcomm and Intel/AMD graphics
firmware from Debian's signed firmware repository. Tests exercise the private
Wi-Fi configuration with a nonexistent fixture network; real radio association
remains a physical hardware acceptance item.
