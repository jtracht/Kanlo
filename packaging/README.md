# Packaging status

`systemd/kanlo-server.service` is a development template, not a tested installer.
The accompanying `sysusers.d/kanlo.conf` configures a non-login `kanlo` user/group;
the unit assumes this account and a binary at
`/usr/local/lib/kanlo/kanlo-server`. It binds loopback and does not configure
LAN ingress, external auth, reverse proxy, desktop, kiosk or TV.

Do not enable a TV service until a native renderer exists. KAN-008–011 and
KAN-061–065 remain open. No install.sh, flash image, update or rollback is
represented as ready. `tests/` checks non-root Debian 13 startup without a
network or display, restart, private storage and open-stream shutdown. Cross
builds cover all three target architectures. Docker is used only for validation;
the Kanlo runtime has no Docker dependency.
