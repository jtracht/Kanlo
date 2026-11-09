# Packaging status

`systemd/kanlo-server.service` is a development template, not a tested installer.
The accompanying `sysusers.d/kanlo.conf` configures a non-login `kanlo` user/group;
the unit assumes this account and a binary at
`/usr/local/lib/kanlo/kanlo-server`. It binds loopback and does not configure
LAN ingress, external auth, reverse proxy, desktop, kiosk or TV.

The tag pipeline builds private development archives for AMD64, ARM64 and armhf.
They include the server, embedded web assets, license notices, build provenance
and [German](../release/INSTALL-DE.md)/[English](../release/INSTALL-EN.md) manual
installation instructions. The downloaded AMD64 archive is tested as a non-root
user with no network or display, including restart, private storage and
open-stream shutdown. Docker is used only for validation; the Kanlo runtime has
no Docker dependency. See [release automation](../docs/de/RELEASES.md).

`images/` builds dedicated Raspberry Pi ARM32/ARM64 and generic UEFI AMD64/ARM64
appliances, including an initial native read-only SDL/Wayland board viewer.
See [image setup and limits](../release/IMAGES-EN.md). Two real QEMU boots of an
isolated image copy are required before release. Physical hardware acceptance,
the complete TV feature set, install.sh and automated update/rollback remain
open; successful image automation does not close those gates.
