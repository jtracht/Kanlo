"""Small, local appliance administration; no remotely exposed setup daemon."""
import argparse
import getpass
import ipaddress
import json
import os
from pathlib import Path
import re
import secrets
import string
import subprocess
import tempfile
import time
import uuid

STATE = Path('/var/lib/kanlo-appliance')
BOOT = Path('/boot/firmware')
DISPLAY_CONFIG = Path('/etc/kanlo/display.json')
DRM = Path('/sys/class/drm')
WIFI_COUNTRY = Path('/etc/modprobe.d/kanlo-wifi-country.conf')
HOTSPOT = Path('/run/kanlo-hotspot')
HOTSPOT_PROFILE = Path('/run/NetworkManager/system-connections/kanlo-setup.nmconnection')
HOTSPOT_ID = 'kanlo-setup-hotspot'
HOTSPOT_SERVICE = 'kanlo-hotspot.service'
HOTSPOT_URL = 'http://10.42.0.1:8080'


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


def set_wifi_country(country):
    if not re.fullmatch(r'[A-Z]{2}', country):
        raise ValueError('Invalid Wi-Fi country')
    if Path('/usr/sbin/iw').exists():
        run('modprobe', 'cfg80211')
        run('iw', 'reg', 'set', country)
    atomic(WIFI_COUNTRY, 'options cfg80211 ieee80211_regdom=' + country + '\n', 0o644)


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
        set_wifi_country(wifi['country'])
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


def display_connectors():
    for status in sorted(DRM.glob('card*-*/status')):
        try:
            state = status.read_text().strip()
        except OSError:
            # A DRM connector can disappear while it is being inspected.
            continue
        try:
            modes = (status.parent / 'modes').read_text().splitlines()
        except OSError:
            modes = []
        yield status.parent.name, state, modes


def display_enabled():
    try:
        return json.loads(DISPLAY_CONFIG.read_text())['enabled'] is True
    except (FileNotFoundError, ValueError, KeyError):
        return False


def has_display():
    return display_enabled() and (graphical_output() or Path('/dev/fb0').exists())


def graphical_output():
    return any(state == 'connected' or ('-Composite-' in name and state == 'unknown' and modes)
               for name, state, modes in display_connectors())


def print_web_addresses():
    try:
        urls = json.loads(Path('/run/kanlo-network/urls.json').read_text())
        print('Web: ' + '  '.join(urls))
    except FileNotFoundError:
        pass


def setup_wifi():
    run('systemctl', 'stop', HOTSPOT_SERVICE)
    country = input('WLAN-Land (z. B. DE, leer = unverändert): ').strip().upper()
    if country:
        set_wifi_country(country)
    run('nmcli', 'radio', 'wifi', 'on')
    run('/usr/bin/nmtui-connect')
    run('systemctl', 'restart', 'kanlo-network.service')
    run('nmcli', '-f', 'DEVICE,TYPE,STATE,CONNECTION', 'device', 'status')
    print_web_addresses()


def setup_device():
    run('systemctl', 'stop', HOTSPOT_SERVICE)
    current_host = Path('/etc/hostname').read_text().strip()
    current_zone = os.path.realpath('/etc/localtime').removeprefix('/usr/share/zoneinfo/')
    if not (Path('/usr/share/zoneinfo') / current_zone).is_file() or current_zone.startswith('/'):
        current_zone = 'Europe/Berlin'
    hostname = input(f'Hostname [{current_host}]: ').strip() or current_host
    timezone = input(f'Zeitzone [{current_zone}]: ').strip() or current_zone
    ssid = input('WLAN-SSID (leer = unverändert): ').strip()
    wifi = {'ssid': ssid, 'password': getpass.getpass('WLAN-Kennwort: '), 'country': input('WLAN-Land [DE]: ').strip() or 'DE'} if ssid else None
    key = input('SSH Public Key (leer = nicht einrichten): ').strip() or None
    display = input('Lokale Anzeige aktivieren? [J/n]: ').strip().lower() != 'n'
    apply({'hostname': hostname, 'timezone': timezone, 'wifi': wifi, 'ssh_authorized_key': key, 'display': display})
    run('nmcli', 'connection', 'reload')
    if wifi:
        run('nmcli', 'connection', 'up', 'kanlo-wifi', capture_output=True)
    run('systemctl', 'restart', 'kanlo-network.service')
    run('systemctl', '--no-block', 'restart', 'kanlo-tv.service')
    print('Gespeichert.')
    print_web_addresses()


def hotspot_start():
    if run('ip', '-4', 'route', 'show', 'scope', 'link', capture_output=True).stdout.strip():
        raise ValueError('A LAN connection is already active')
    run('nmcli', 'radio', 'wifi', 'on')
    devices = run('nmcli', '-t', '-f', 'DEVICE,TYPE,STATE', 'device', 'status',
                  capture_output=True, env={**os.environ, 'LC_ALL': 'C'}).stdout.splitlines()
    interface = next((row.split(':')[0] for row in devices
                      if re.fullmatch(r'[a-zA-Z0-9_.-]+:wifi:disconnected', row)), None)
    if interface is None:
        raise ValueError('No disconnected Wi-Fi adapter is available')
    ssid = 'Kanlo-' + secrets.token_hex(3)
    password = ''.join(secrets.choice(string.ascii_letters + string.digits) for _ in range(16))
    profile_id = str(uuid.uuid4())
    atomic(HOTSPOT / 'uuid', profile_id)
    atomic(HOTSPOT_PROFILE, f'[connection]\nid={HOTSPOT_ID}\nuuid={profile_id}\n'
           f'type=wifi\ninterface-name={interface}\nautoconnect=false\n'
           f'[wifi]\nmode=ap\nband=bg\nssid={ssid}\n'
           f'[wifi-security]\nkey-mgmt=wpa-psk\nproto=rsn;\npsk={password}\n'
           '[ipv4]\nmethod=shared\naddress1=10.42.0.1/24\n[ipv6]\nmethod=disabled\n')
    # Do not place credentials in process arguments, standard output or the journal.
    run('nmcli', 'connection', 'load', str(HOTSPOT_PROFILE), capture_output=True)
    run('nmcli', '--wait', '30', 'connection', 'up', 'uuid', profile_id, capture_output=True)
    run('systemctl', 'restart', 'kanlo-network.service')
    atomic(HOTSPOT / 'credentials.json', json.dumps({'ssid': ssid, 'password': password}))


def hotspot_stop():
    profile = HOTSPOT / 'uuid'
    if profile.is_file():
        profile_id = str(uuid.UUID(profile.read_text()))
        subprocess.run(['nmcli', '--wait', '10', 'connection', 'delete', 'uuid', profile_id],
                       check=False, capture_output=True)
    HOTSPOT_PROFILE.unlink(missing_ok=True)
    (HOTSPOT / 'credentials.json').unlink(missing_ok=True)
    subprocess.run(['systemctl', '--no-block', 'start', 'kanlo-network.service'],
                   check=False, capture_output=True)


def hotspot_serve():
    hotspot_start()
    run('systemd-notify', '--ready')
    time.sleep(600)


def setup_hotspot():
    if not (HOTSPOT / 'credentials.json').is_file():
        run('systemctl', 'start', HOTSPOT_SERVICE)
    credentials = json.loads((HOTSPOT / 'credentials.json').read_text())
    print(f"Temporäres WLAN: {credentials['ssid']}\nKennwort: {credentials['password']}")
    print(f'Kanlo im Browser: {HOTSPOT_URL}\nDer Hotspot endet nach spätestens 10 Minuten.')


def setup():
    while True:
        print('\033[2J\033[HKanlo – lokale Einrichtung (Strg+Alt+F1: Board)')
        print_web_addresses()
        print('\n1  WLAN auswählen und verbinden\n2  Gerät einrichten\n3  Anzeige-Diagnose\n4  Board-Anzeige neu starten'
              '\n5  Temporären WLAN-Hotspot öffnen\n6  Hotspot schließen')
        choice = input('\nAuswahl: ').strip()
        try:
            if choice == '1':
                setup_wifi()
            elif choice == '2':
                setup_device()
            elif choice == '3':
                for name, state, modes in display_connectors():
                    print(f'{name}: {state} ({", ".join(modes) or "keine Modi"})')
                run('journalctl', '-b', '-u', 'kanlo-tv.service', '-n', '20', '--no-pager')
            elif choice == '4':
                run('systemctl', '--no-block', 'restart', 'kanlo-tv.service')
            elif choice == '5':
                setup_hotspot()
            elif choice == '6':
                run('systemctl', 'stop', HOTSPOT_SERVICE)
            else:
                continue
        except (OSError, ValueError, subprocess.CalledProcessError):
            print('Einrichtung fehlgeschlagen. Eingaben und Geräte prüfen.')
        input('Enter zurück zum Menü …')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('command', choices=['firstboot', 'network', 'has-display', 'setup', 'hotspot-start', 'hotspot-stop'])
    command = parser.parse_args().command
    if command == 'has-display':
        raise SystemExit(0 if has_display() else 1)
    {'firstboot': firstboot, 'network': network, 'setup': setup,
     'hotspot-start': hotspot_serve, 'hotspot-stop': hotspot_stop}[command]()
