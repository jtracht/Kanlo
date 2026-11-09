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

Do not enable a TV service until a native renderer exists. No install.sh, flash
image or automated update/rollback is represented as ready. Cross builds do not
certify physical ARM devices. The release pipeline does not close those gates.
