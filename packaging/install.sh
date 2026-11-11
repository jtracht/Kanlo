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
    '') ;;
    *) printf '%s\n' 'Usage: install.sh [--check]' >&2; exit 2 ;;
esac
if [ "$(id -u)" -ne 0 ]; then
    printf '%s\n' 'Run sudo ./install.sh to install the server service.' >&2
    exit 1
fi

install -d -m 0755 /usr/local/lib/kanlo
new_binary=$(mktemp /usr/local/lib/kanlo/.kanlo-server.XXXXXX)
trap 'rm -f "$new_binary"' EXIT
install -m 0755 bin/kanlo-server "$new_binary"
install -Dm644 systemd/kanlo-server.service /etc/systemd/system/kanlo-server.service
install -Dm644 sysusers.d/kanlo.conf /usr/lib/sysusers.d/kanlo.conf
systemd-sysusers /usr/lib/sysusers.d/kanlo.conf
mv -f "$new_binary" /usr/local/lib/kanlo/kanlo-server
systemctl daemon-reload
systemctl enable kanlo-server.service
systemctl restart kanlo-server.service
