# Packaging status

`systemd/kanlo-server.service` is a development template, not a tested installer.
It assumes a pre-existing `kanlo` user/group and binary at
`/usr/local/lib/kanlo/kanlo-server`. It binds loopback and does not configure
LAN ingress, external auth, reverse proxy, desktop, kiosk or TV.

Do not enable a TV service until a native renderer exists. KAN-008–011 and
KAN-061–065 remain open. No install.sh, flash image, update or rollback is
represented as ready. Docker is used only by the optional CI build environment;
the Kanlo runtime has no Docker dependency.
