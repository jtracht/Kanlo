"""Small, local appliance administration; no remotely exposed setup daemon."""
import argparse
import getpass
import ipaddress
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time

STATE = Path('/var/lib/kanlo-appliance')
BOOT = Path('/boot/firmware')
DISPLAY_CONFIG = Path('/etc/kanlo/display.json')


def run(*args, **kwargs):
    return subprocess.run(args, check=True, text=True, **kwargs)


def validate(config):
    if not isinstance(config, dict) or set(config) - {'hostname', 'timezone', 'display', 'wifi', 'ssh_authorized_key'}:
        raise ValueError('Unknown configuration field')
    hostname = config.get('hostname', 'kanlo')
    if not isinstance(hostname, str) or not re.fullmatch(r'[a-z][a-z0-9-]{0,61}[a-z0-9]|[a-z]', hostname):
        raise ValueError('Invalid hostname')
    timezone = config.get('timezone', 'Etc/UTC')
    if not isinstance(timezone, str) or not re.fullmatch(r'[A-Za-z0-9_+-]+(?:/[A-Za-z0-9_+-]+)*', timezone) or not (Path('/usr/share/zoneinfo') / timezone).is_file():
        raise ValueError('Invalid timezone')
    if not isinstance(config.get('display', True), bool):
        raise ValueError('display must be true or false')
    wifi = config.get('wifi')
    if wifi is not None:
        if not isinstance(wifi, dict) or set(wifi) != {'ssid', 'password', 'country'}:
            raise ValueError('wifi requires ssid, password and country')
        if not isinstance(wifi['ssid'], str) or not 1 <= len(wifi['ssid'].encode()) <= 32 or any(c in wifi['ssid'] for c in '\r\n\0'):
            raise ValueError('Invalid Wi-Fi SSID')
        if not isinstance(wifi['password'], str) or not 8 <= len(wifi['password']) <= 63 or any(c in wifi['password'] for c in '\r\n\0'):
            raise ValueError('Wi-Fi password needs 8–63 characters')
        if not isinstance(wifi['country'], str) or not re.fullmatch(r'[A-Z]{2}', wifi['country']):
            raise ValueError('Invalid Wi-Fi country')
    key = config.get('ssh_authorized_key')
    if key is not None and (not isinstance(key, str) or '\n' in key or '\r' in key or len(key) > 16384 or not re.fullmatch(r'(ssh-ed25519|ssh-rsa|ecdsa-sha2-nistp256) [A-Za-z0-9+/=]+(?: [^\r\n]*)?', key)):
        raise ValueError('Provide one plain SSH public key (no authorized_keys options)')
    return {'hostname': hostname, 'timezone': timezone, 'display': config.get('display', True), 'wifi': wifi, 'ssh_authorized_key': key}


def atomic(path, content, mode=0o600):
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(dir=path.parent, prefix='.kanlo-')
    try:
        with os.fdopen(fd, 'w', encoding='utf-8') as stream:
            stream.write(content)
        os.chmod(temporary, mode)
        os.replace(temporary, path)
    finally:
        Path(temporary).unlink(missing_ok=True)


def apply(config):
    config = validate(config)
    atomic(Path('/etc/hostname'), config['hostname'] + '\n', 0o644)
    run('hostname', config['hostname'])
    atomic(Path('/etc/hosts'), f"127.0.0.1 localhost\n127.0.1.1 {config['hostname']}\n::1 localhost ip6-localhost\n", 0o644)
    Path('/etc/localtime').unlink(missing_ok=True)
    Path('/etc/localtime').symlink_to('/usr/share/zoneinfo/' + config['timezone'])
    atomic(DISPLAY_CONFIG, json.dumps({'enabled': config['display']}), 0o644)
    wifi = config['wifi']
    if wifi:
        # NetworkManager keyfile escaping, not shell interpolation. No password in argv/logs.
        def escape(value):
            return value.replace('\\', '\\\\').replace(';', '\\;').replace(' ', '\\s')
        atomic(Path('/etc/NetworkManager/system-connections/kanlo-wifi.nmconnection'),
               '[connection]\nid=kanlo-wifi\ntype=wifi\nautoconnect=true\n'
               '[wifi]\nmode=infrastructure\nssid=' + ';'.join(str(c) for c in wifi['ssid'].encode()) + ';\n'
               '[wifi-security]\nkey-mgmt=wpa-psk\npsk=' + escape(wifi['password']) + '\n'
               '[ipv4]\nmethod=auto\n[ipv6]\nmethod=auto\n')
        atomic(Path('/etc/modprobe.d/kanlo-wifi-country.conf'), 'options cfg80211 ieee80211_regdom=' + wifi['country'] + '\n', 0o644)
        if Path('/usr/sbin/iw').exists():
            run('iw', 'reg', 'set', wifi['country'])
    key = config['ssh_authorized_key']
    if key:
        # No password login; only the supplied device-owner key enables SSH.
        if not user_exists('kanlo-admin'):
            run('useradd', '--create-home', '--shell', '/bin/bash', 'kanlo-admin')
        home = Path('/home/kanlo-admin/.ssh'); home.mkdir(mode=0o700, parents=True, exist_ok=True)
        atomic(home / 'authorized_keys', key + '\n')
        run('chown', '-R', 'kanlo-admin:kanlo-admin', str(home))
        # A non-password account marker permits public-key authentication without a shared password.
        run('usermod', '--password', '*', 'kanlo-admin')
        atomic(Path('/etc/sudoers.d/kanlo-admin'), 'kanlo-admin ALL=(ALL:ALL) NOPASSWD: ALL\n', 0o440)
        run('ssh-keygen', '-A')
        atomic(Path('/etc/ssh/sshd_config.d/00-kanlo.conf'), 'PasswordAuthentication no\nKbdInteractiveAuthentication no\nPermitRootLogin no\nAllowUsers kanlo-admin\n', 0o644)
        run('systemctl', 'enable', '--now', 'ssh.service')
    STATE.mkdir(mode=0o700, parents=True, exist_ok=True)
    # Retain no Wi-Fi secret or private provision file on the FAT boot partition.
    atomic(STATE / 'provisioned.json', json.dumps({'schema_version': 1, 'hostname': config['hostname'], 'configured_at': int(time.time()), 'ssh_enabled': bool(key)}))


def user_exists(name):
    import pwd
    try:
        pwd.getpwnam(name)
        return True
    except KeyError:
        return False


def grow():
    source = os.path.realpath(run('findmnt', '-n', '-o', 'SOURCE', '/', capture_output=True).stdout.strip())
    disk = run('lsblk', '-n', '-o', 'PKNAME', source, capture_output=True).stdout.strip()
    number = run('lsblk', '-n', '-o', 'PARTN', source, capture_output=True).stdout.strip()
    if not re.fullmatch(r'[a-zA-Z0-9_-]+', disk) or not number.isdigit():
        raise ValueError('Root must be a disk partition')
    result = subprocess.run(['growpart', '/dev/' + disk, number], capture_output=True, text=True)
    if result.returncode and 'NOCHANGE:' not in result.stdout:
        raise RuntimeError('Cannot expand root partition: ' + result.stdout + result.stderr)
    run('resize2fs', source)


def firstboot():
    grow()
    config_file = BOOT / 'KANLO_CONFIG.json'
    if config_file.exists():
        if config_file.stat().st_size > 32768:
            raise ValueError('Provision file too large')
        config = json.loads(config_file.read_text(encoding='utf-8'))
        apply(config)
        config_file.unlink()
    else:
        suffix = Path('/etc/machine-id').read_text().strip()[-6:]
        apply({'hostname': 'kanlo-' + suffix})


def authorities(addresses, hostname):
    hosts = {f'{hostname}:8080', f'{hostname}.local:8080'}
    for value in addresses:
        address = ipaddress.ip_address(value)
        if address.version == 4 and (address.is_private or address.is_link_local) and not address.is_loopback:
            hosts.add(f'{address}:8080')
    return sorted(hosts)


def network():
    info = json.loads(run('ip', '-j', '-4', 'address', 'show', capture_output=True).stdout)
    addresses = [a['local'] for interface in info for a in interface.get('addr_info', []) if a.get('family') == 'inet']
    hostname = Path('/etc/hostname').read_text().strip()
    hosts = authorities(addresses, hostname)
    environment = Path('/run/kanlo-network/server.env')
    content = 'KANLO_BIND=0.0.0.0:8080\nKANLO_ALLOWED_HOSTS=' + ','.join(hosts) + '\n'
    changed = not environment.exists() or environment.read_text() != content
    if changed:
        atomic(environment, content, 0o644)
    atomic(Path('/run/kanlo-network/urls.json'), json.dumps(['http://' + h for h in hosts if '.local:' not in h and not h.startswith(hostname + ':')] or ['http://' + hostname + '.local:8080']), 0o644)
    if changed and subprocess.run(['systemctl', 'is-active', '--quiet', 'kanlo-server.service']).returncode == 0:
        run('systemctl', '--no-block', 'try-restart', 'kanlo-server.service')


def has_display():
    try:
        if not json.loads(DISPLAY_CONFIG.read_text())['enabled']:
            return False
    except FileNotFoundError:
        return False
    return any(path.read_text().strip() == 'connected' for path in Path('/sys/class/drm').glob('card*-*/status'))


def setup():
    print('\033[2J\033[HKanlo – lokale Einrichtung / local setup (Ctrl+Alt+F1: Anzeige)')
    try:
        urls = json.loads(Path('/run/kanlo-network/urls.json').read_text())
        print('\nWeb: ' + '  '.join(urls))
    except FileNotFoundError:
        pass
    print('Kein voreingestelltes Kennwort. Ethernet: automatisch per DHCP.\n')
    hostname = input('Hostname [kanlo]: ').strip() or 'kanlo'
    timezone = input('Zeitzone [Europe/Berlin]: ').strip() or 'Europe/Berlin'
    ssid = input('WLAN-SSID (leer = unverändert): ').strip()
    wifi = {'ssid': ssid, 'password': getpass.getpass('WLAN-Kennwort: '), 'country': input('WLAN-Land [DE]: ').strip() or 'DE'} if ssid else None
    key = input('SSH Public Key (leer = nicht einrichten): ').strip() or None
    display = input('Lokale Anzeige aktivieren? [J/n]: ').strip().lower() != 'n'
    try:
        apply({'hostname': hostname, 'timezone': timezone, 'wifi': wifi, 'ssh_authorized_key': key, 'display': display})
        run('nmcli', 'connection', 'reload')
        if wifi:
            run('nmcli', 'connection', 'up', 'kanlo-wifi')
        run('systemctl', 'restart', 'kanlo-network.service')
        run('systemctl', 'restart', 'kanlo-server.service')
        run('systemctl', '--no-block', 'restart', 'kanlo-tv.service')
        print('Gespeichert. Web-Adresse erscheint auf der Anzeige.')
    except (ValueError, subprocess.CalledProcessError):
        print('Einrichtung fehlgeschlagen. Eingaben und Netzwerk prüfen; keine Zugangsdaten protokolliert.')
    input('Enter für erneute Einrichtung …')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('command', choices=['firstboot', 'network', 'has-display', 'setup'])
    command = parser.parse_args().command
    if command == 'has-display':
        raise SystemExit(0 if has_display() else 1)
    {'firstboot': firstboot, 'network': network, 'setup': setup}[command]()
