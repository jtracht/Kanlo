#!/bin/sh
set -eu
export DEBIAN_FRONTEND=noninteractive
printf '#!/bin/sh\nexit 101\n' > /usr/sbin/policy-rc.d
chmod 755 /usr/sbin/policy-rc.d
set --
if [ "${KANLO_IMAGE_FAMILY:-}" = generic ]; then
    # Device firmware is installed now, so first Wi-Fi/display use needs no download.
    cat > /etc/apt/sources.list.d/kanlo-firmware.sources <<'SOURCES'
Types: deb
URIs: https://deb.debian.org/debian
Suites: trixie trixie-updates
Components: non-free-firmware
Signed-By: /usr/share/keyrings/debian-archive-keyring.gpg

Types: deb
URIs: https://deb.debian.org/debian-security
Suites: trixie-security
Components: non-free-firmware
Signed-By: /usr/share/keyrings/debian-archive-keyring.gpg
SOURCES
    set -- firmware-linux-free firmware-iwlwifi firmware-realtek firmware-brcm80211 \
        firmware-atheros firmware-misc-nonfree firmware-amd-graphics firmware-intel-graphics
fi
apt-get update
apt-get install -y --no-install-recommends python3 python3-pygame cage seatd \
    fonts-dejavu-core network-manager avahi-daemon systemd-resolved \
    cloud-guest-utils parted systemd-zram-generator sudo openssh-server iw \
    wpasupplicant wireless-regdb dnsmasq-base bluez rfkill kbd \
    python3-qrcode python3-markdown-it python3-pil curl ca-certificates "$@"
# No cloud-provider startup or first-user wizard in the dedicated appliance.
install -d -m 0755 /etc/cloud
: > /etc/cloud/cloud-init.disabled
for unit in cloud-init-local.service cloud-init.service cloud-config.service cloud-final.service \
    cloud-init-network.service cloud-init-main.service sshswitch.service userconfig.service userconf-pi.service \
    getty@tty1.service getty@tty2.service serial-getty@ttyS0.service \
    serial-getty@ttyAMA0.service ssh.service ssh.socket dphys-swapfile.service \
    rpi-connect.service rpi-connect-lite.service ModemManager.service \
    rpi-resize-swap-file.service rpi-zram-writeback.service \
    systemd-networkd.service systemd-networkd-wait-online.service networking.service dhcpcd.service; do
    systemctl mask "$unit"
done
# SSH is disabled, but can be explicitly enabled by first-boot provisioning.
systemctl unmask ssh.service
systemctl disable ssh.service
systemctl enable NetworkManager.service avahi-daemon.service systemd-resolved.service seatd.service bluetooth.service
systemctl disable systemd-networkd.service systemd-networkd-wait-online.service 2>/dev/null || true
systemctl enable kanlo-firstboot.service kanlo-network.service kanlo-network.timer kanlo-server.service kanlo-tv.service kanlo-setup.service
systemctl set-default multi-user.target
id kanlo >/dev/null 2>&1 || useradd --system --no-create-home --shell /usr/sbin/nologin kanlo
id kanlo-display >/dev/null 2>&1 || useradd --system --no-create-home --shell /usr/sbin/nologin kanlo-display
getent group render >/dev/null || groupadd --system render
getent group video >/dev/null || groupadd --system video
usermod -a -G video,render kanlo-display
passwd -l root
for account in pi debian; do
    if id "$account" >/dev/null 2>&1; then usermod --lock --shell /usr/sbin/nologin "$account"; fi
done
install -d -m 0700 -o kanlo -g kanlo /var/lib/kanlo
install -d -m 0700 /var/lib/kanlo-appliance
install -d -m 0755 /boot/firmware /etc/kanlo
# No shared machine identity, SSH host keys, random seed or DHCP leases.
truncate -s 0 /etc/machine-id
rm -f /var/lib/dbus/machine-id /var/lib/systemd/random-seed /etc/ssh/ssh_host_* \
    /var/lib/NetworkManager/*lease* /var/lib/NetworkManager/secret_key /var/lib/kanlo-appliance/provisioned.json
ln -s /etc/machine-id /var/lib/dbus/machine-id
printf '[connection]\nid=kanlo-ethernet\ntype=ethernet\nautoconnect=true\n[ipv4]\nmethod=auto\n[ipv6]\nmethod=auto\n' > /etc/NetworkManager/system-connections/kanlo-ethernet.nmconnection
chmod 600 /etc/NetworkManager/system-connections/kanlo-ethernet.nmconnection
rm -f /etc/resolv.conf
ln -s /run/systemd/resolve/stub-resolv.conf /etc/resolv.conf
rm -f /var/swap
# Prevent persistent SD-card swap; use bounded compressed RAM instead.
sed -i '/^[^#].*[[:space:]]swap[[:space:]]/s/^/# Kanlo: /' /etc/fstab
printf 'auto lo\niface lo inet loopback\n' > /etc/network/interfaces
python3 - <<'PY'
from pathlib import Path
path = Path('/etc/fstab')
lines = []
for line in path.read_text().splitlines():
    fields = line.split()
    if fields and not fields[0].startswith('#') and len(fields) >= 4 and fields[1] == '/' and fields[2] == 'ext4':
        options = [option for option in fields[3].split(',') if option not in ('atime', 'relatime', 'strictatime', 'noatime')]
        fields[3] = ','.join(options + ['noatime'])
        line = '\t'.join(fields)
    lines.append(line)
path.write_text('\n'.join(lines) + '\n')
PY
dpkg-query -W -f='${Package}\t${Version}\t${Architecture}\n' > /usr/share/kanlo/image-packages.tsv
apt-get clean
rm -rf /var/lib/apt/lists/* /var/log/journal/*
rm -f /usr/sbin/policy-rc.d
