#!/bin/sh
set -eu

test -f /.dockerenv
test "$(id -u)" -eq 0
test ! -e /etc/kanlo/standalone-install
package=$1
work=$(mktemp -d)
mkdir "$work/commands"
export KANLO_TEST_LOG="$work/systemctl.log"
cat > "$work/commands/systemctl" <<'SH'
#!/bin/sh
printf '%s\n' "$*" >> "$KANLO_TEST_LOG"
if [ "$1" = is-active ]; then
    test "${KANLO_TEST_DESKTOP:-off}" = on
fi
SH
cat > "$work/commands/systemd-sysusers" <<'SH'
#!/bin/sh
test -f "$1"
SH
chmod +x "$work/commands/"*
export PATH="$work/commands:$PATH"
tar -xzf "$package" -C "$work"
cd "$work"/kanlo-*-linux-amd64

desktop=/etc/systemd/system/display-manager.service
mkdir -p /etc/systemd/system /var/lib/kanlo
printf 'desktop fixture\n' > "$desktop"
printf 'persistent data\n' > /var/lib/kanlo/test-sentinel
./install.sh --check
./install.sh
test -f /etc/systemd/system/kanlo-server.service
test -f /usr/lib/kanlo/update.py
test ! -e /etc/systemd/system/kanlo-tv.service
test "$(cat "$desktop")" = 'desktop fixture'
! grep -q 'kanlo-tv.service' "$KANLO_TEST_LOG"
if KANLO_TEST_DESKTOP=on ./install.sh --kiosk; then
    printf '%s\n' 'Kiosk must refuse an active desktop.' >&2
    exit 1
fi
test ! -e /etc/systemd/system/kanlo-tv.service
./install.sh --kiosk
test -f /etc/systemd/system/kanlo-tv.service
test -f /usr/local/lib/kanlo/kanlo-server.previous
grep -Fx 'enable --now seatd.service kanlo-tv.service' "$KANLO_TEST_LOG"
./install.sh --uninstall
test ! -e /etc/systemd/system/kanlo-tv.service
test ! -e /etc/systemd/system/kanlo-server.service
test ! -e /usr/lib/kanlo/update.py
test "$(cat /var/lib/kanlo/test-sentinel)" = 'persistent data'
test "$(cat "$desktop")" = 'desktop fixture'
printf '%s\n' 'PASS: standalone install, isolated kiosk mode, upgrade and data-preserving uninstall'
