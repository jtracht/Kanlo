#!/bin/sh
set -eu

cd "$(dirname "$0")"
. /etc/os-release
case "${ID:-}:${VERSION_ID:-}" in
    debian:13|raspbian:13) ;;
    *) printf '%s\n' 'Kanlo requires Debian or Raspberry Pi OS 13.' >&2; exit 1 ;;
esac

arch=$(sed -n 's/^  "arch": "\([a-z0-9]*\)",$/\1/p' BUILD.json)
if [ "$arch" != "$(dpkg --print-architecture)" ]; then
    printf 'Package architecture %s does not match this OS.\n' "$arch" >&2
    exit 1
fi
checksum=$(sed -n 's/^  "binary_sha256": "\([0-9a-f]*\)",$/\1/p' BUILD.json)
printf '%s  bin/kanlo-server\n' "$checksum" | sha256sum --check --status

case "${1:-}" in
    --check) printf '%s\n' 'Kanlo package verified.'; exit 0 ;;
    ''|--kiosk|--uninstall) ;;
    *) printf '%s\n' 'Usage: install.sh [--check|--kiosk|--uninstall]' >&2; exit 2 ;;
esac
if [ "$(id -u)" -ne 0 ]; then
    printf '%s\n' 'Run sudo ./install.sh to install the server service.' >&2
    exit 1
fi

mode=${1:-}
marker=/etc/kanlo/standalone-install
if [ -f /etc/systemd/system/kanlo-firstboot.service ]; then
    printf '%s\n' 'Use the appliance update procedure for a dedicated Kanlo image.' >&2
    exit 1
fi
if [ "$mode" = --uninstall ]; then
    if [ ! -f "$marker" ]; then
        printf '%s\n' 'No standalone installation marker found.' >&2
        exit 1
    fi
    if [ -f /etc/systemd/system/kanlo-tv.service ]; then
        systemctl disable --now kanlo-tv.service
        rm -f /etc/systemd/system/kanlo-tv.service /etc/systemd/system/kanlo-tv.service.d/standalone.conf \
            /usr/lib/kanlo/display.py /usr/lib/kanlo/tv_markdown.py /usr/lib/kanlo/tv_wayland.py /usr/lib/kanlo/appliance.py
    fi
    if [ -f /etc/systemd/system/kanlo-server.service ]; then
        systemctl disable --now kanlo-server.service
    fi
    rm -f "$marker" /etc/systemd/system/kanlo-server.service /usr/lib/sysusers.d/kanlo.conf \
        /usr/local/lib/kanlo/kanlo-server /usr/local/lib/kanlo/kanlo-server.previous /usr/lib/kanlo/update.py
    systemctl daemon-reload
    printf '%s\n' 'Kanlo services removed. Data and configuration remain in place.'
    exit 0
fi

if [ "$mode" = --kiosk ]; then
    if systemctl is-active --quiet display-manager.service; then
        printf '%s\n' 'Stop the active desktop display manager before installing the dedicated kiosk.' >&2
        exit 1
    fi
    if ! command -v weston >/dev/null || ! command -v cage >/dev/null || ! command -v seatd >/dev/null ||
        ! python3 -c 'import cairo, gi, pygame; gi.require_version("Gtk", "3.0")' >/dev/null 2>&1; then
        printf '%s\n' 'Install weston, cage, seatd, python3-pygame, python3-gi-cairo, gir1.2-gtk-3.0 and fonts-dejavu-core first.' >&2
        exit 1
    fi
fi

install -d -m 0755 /usr/local/lib/kanlo
install -d -m 0755 /etc/kanlo
new_binary=$(mktemp /usr/local/lib/kanlo/.kanlo-server.XXXXXX)
trap 'rm -f "$new_binary"' EXIT
install -m 0755 bin/kanlo-server "$new_binary"
install -Dm644 systemd/kanlo-server.service /etc/systemd/system/kanlo-server.service
install -Dm644 sysusers.d/kanlo.conf /usr/lib/sysusers.d/kanlo.conf
install -Dm644 update.py /usr/lib/kanlo/update.py
systemd-sysusers /usr/lib/sysusers.d/kanlo.conf
if [ -f /usr/local/lib/kanlo/kanlo-server ]; then
    install -m 0755 /usr/local/lib/kanlo/kanlo-server /usr/local/lib/kanlo/kanlo-server.previous
fi
mv -f "$new_binary" /usr/local/lib/kanlo/kanlo-server
: > "$marker"
if [ "$mode" = --kiosk ]; then
    getent group render >/dev/null || groupadd --system render
    id kanlo-display >/dev/null 2>&1 || useradd --system --no-create-home --shell /usr/sbin/nologin kanlo-display
    usermod -a -G video,render kanlo-display
    install -Dm644 kiosk/display.py /usr/lib/kanlo/display.py
    install -Dm644 kiosk/tv_markdown.py /usr/lib/kanlo/tv_markdown.py
    install -Dm644 kiosk/tv_wayland.py /usr/lib/kanlo/tv_wayland.py
    install -Dm644 kiosk/appliance.py /usr/lib/kanlo/appliance.py
    install -Dm644 kiosk/kanlo-tv.service /etc/systemd/system/kanlo-tv.service
    install -d -m 0755 /etc/systemd/system/kanlo-tv.service.d
    printf '%s\n' '[Service]' 'Environment=KANLO_DISPLAY_ORIGIN=http://127.0.0.1:8080' \
        > /etc/systemd/system/kanlo-tv.service.d/standalone.conf
    if [ ! -e /etc/kanlo/display.json ]; then
        printf '%s\n' '{"enabled":true}' > /etc/kanlo/display.json
        chmod 0644 /etc/kanlo/display.json
    fi
fi
systemctl daemon-reload
systemctl enable kanlo-server.service
systemctl restart kanlo-server.service
if [ "$mode" = --kiosk ]; then
    systemctl enable --now seatd.service kanlo-tv.service
fi
