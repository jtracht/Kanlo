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
HOTSPOT_URL = 'http://10.42.0.1'
KEYBOARD = Path('/etc/default/keyboard')
KEYBOARD_LAYOUTS = (
    ('de', '', 'Deutsch (Deutschland)'),
    ('ch', '', 'Deutsch (Schweiz)'),
    ('us', '', 'Englisch (USA)'),
    ('gb', '', 'Englisch (UK)'),
    ('fr', '', 'Französisch (Frankreich)'),
    ('ch', 'fr', 'Französisch (Schweiz)'),
)


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
    atomic(STATE / 'provisioned.json', json.dumps({'schema_version': 1, 'hostname': config['hostname'], 'configured_at': int(time.time()), 'ssh_enabled': bool(key), 'wifi_configured': bool(wifi)}))


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
    hosts = {hostname, f'{hostname}.local'}
    for value in addresses:
        address = ipaddress.ip_address(value)
        if address.version == 4 and (address.is_private or address.is_link_local) and not address.is_loopback:
            hosts.add(str(address))
    return sorted(hosts)


def network():
    info = json.loads(run('ip', '-j', '-4', 'address', 'show', capture_output=True).stdout)
    addresses = [a['local'] for interface in info for a in interface.get('addr_info', []) if a.get('family') == 'inet']
    hostname = Path('/etc/hostname').read_text().strip()
    hosts = authorities(addresses, hostname)
    environment = Path('/run/kanlo-network/server.env')
    content = 'KANLO_BIND=0.0.0.0:80\nKANLO_ALLOWED_HOSTS=' + ','.join(hosts) + '\n'
    changed = not environment.exists() or environment.read_text() != content
    if changed:
        atomic(environment, content, 0o644)
    local_hosts = {hostname, f'{hostname}.local'}
    urls = ['http://' + host for host in hosts if host not in local_hosts]
    atomic(Path('/run/kanlo-network/urls.json'), json.dumps(urls or [f'http://{hostname}.local']), 0o644)
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
    return any(state == 'connected' or (state == 'unknown' and modes
               and any(output in name for output in ('-Composite-', '-HDMI-', '-DP-')))
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
    print('WLAN wird aktiviert und nach Netzwerken gesucht …', flush=True)
    prepare_wifi_scan()
    run('/usr/bin/nmtui-connect')
    run('systemctl', 'restart', 'kanlo-network.service')
    run('nmcli', '-f', 'DEVICE,TYPE,STATE,CONNECTION', 'device', 'status')
    print_web_addresses()


def prepare_wifi_scan():
    run('rfkill', 'unblock', 'wifi')
    run('nmcli', '--wait', '5', 'radio', 'wifi', 'on')
    env = {**os.environ, 'LC_ALL': 'C'}
    for attempt in range(15):
        devices = run('nmcli', '--wait', '5', '-t', '-f', 'TYPE,STATE', 'device', 'status',
                      capture_output=True, env=env).stdout.splitlines()
        if any(row.startswith('wifi:') and row not in ('wifi:unavailable', 'wifi:unmanaged')
               for row in devices):
            scan = subprocess.run(
                ['nmcli', '--wait', '12', '-t', '-f', 'SSID', 'device', 'wifi', 'list', '--rescan', 'yes'],
                capture_output=True, text=True, check=False, env=env,
            )
            if scan.returncode == 0:
                return
            # NetworkManager can report the device before scans are allowed.
            # Avoid repeating a full scan that already used its entire timeout.
            if scan.returncode == 3:
                break
        if attempt < 14:
            time.sleep(1)
    raise ValueError('WLAN-Adapter oder Netzwerksuche nicht verfügbar.')


def needs_welcome():
    if not has_display() or (STATE / 'welcome-complete').exists():
        return False
    try:
        configured = json.loads((STATE / 'provisioned.json').read_text())
    except (OSError, ValueError):
        return False
    return not configured.get('wifi_configured', False)


def wifi_ready():
    # NetworkManager may still be assigning an address when its dialog closes.
    for attempt in range(12):
        rows = run('nmcli', '-t', '-f', 'TYPE,STATE', 'device', 'status',
                   capture_output=True, env={**os.environ, 'LC_ALL': 'C'}).stdout.splitlines()
        states = [row.split(':', 1)[1] for row in rows if row.startswith('wifi:')]
        if any(state in ('connected', 'connected (externally)') for state in states):
            return True
        if not any(state.startswith('connecting') for state in states):
            return False
        if attempt < 11:
            time.sleep(1)
    return False


def keyboard_menu():
    print('\nKanlo – Tastaturlayout')
    for number, (_, _, label) in enumerate(KEYBOARD_LAYOUTS, 1):
        print(f'{number}  {label}')
    print('0  Aktuelles Layout behalten')
    while True:
        choice = input('\nAuswahl [0]: ').strip() or '0'
        if choice == '0':
            return
        if choice not in {str(index) for index in range(1, len(KEYBOARD_LAYOUTS) + 1)}:
            continue
        layout, variant, label = KEYBOARD_LAYOUTS[int(choice) - 1]
        previous = KEYBOARD.read_text() if KEYBOARD.exists() else None
        atomic(KEYBOARD, f'XKBMODEL="pc105"\nXKBLAYOUT="{layout}"\nXKBVARIANT="{variant}"\n'
                         'XKBOPTIONS=""\nBACKSPACE="guess"\n', 0o644)
        try:
            run('setupcon', '--keyboard-only', '--force', '--save')
        except (OSError, subprocess.CalledProcessError):
            if previous is None:
                KEYBOARD.unlink(missing_ok=True)
            else:
                atomic(KEYBOARD, previous, 0o644)
            raise
        print(f'Tastaturlayout: {label}')
        return


def welcome():
    run('chvt', '1')
    try:
        keyboard_menu()
    except (OSError, subprocess.CalledProcessError):
        print('Tastaturlayout konnte nicht eingerichtet werden.')
        input('Enter für die WLAN-Ersteinrichtung …')
    while True:
        print('\033[2J\033[HKanlo – WLAN-Ersteinrichtung')
        print('1  WLAN auswählen und verbinden\n0  Weiter zum Board')
        choice = input('\nAuswahl [1]: ').strip() or '1'
        if choice == '1':
            try:
                setup_wifi()
                if wifi_ready():
                    finish_welcome()
                    return
            except (OSError, ValueError, subprocess.CalledProcessError):
                print('WLAN konnte nicht eingerichtet werden.')
            input('Enter für die WLAN-Ersteinrichtung …')
        elif choice == '0':
            finish_welcome()
            return


def finish_welcome():
    atomic(STATE / 'welcome-complete', '1\n')
    # The display service orders itself after this unit. Queue it without waiting.
    run('systemctl', '--no-block', 'start', 'kanlo-tv.service')


def return_to_board(restart=False):
    run('systemctl', '--no-block', 'restart' if restart else 'start', 'kanlo-tv.service')
    run('chvt', '1')


def wifi_menu():
    while True:
        print('\033[2J\033[HKanlo – WLAN')
        print_web_addresses()
        print('\n1  WLAN auswählen und verbinden\n2  Gespeicherte Verbindungen bearbeiten'
              '\n3  Temporären WLAN-Hotspot öffnen\n4  Hotspot schließen\n0  Zurück')
        choice = input('\nAuswahl: ').strip()
        if choice == '0':
            return
        try:
            if choice == '1':
                setup_wifi()
            elif choice == '2':
                run('systemctl', 'stop', HOTSPOT_SERVICE)
                run('/usr/bin/nmtui-edit')
                run('systemctl', 'restart', 'kanlo-network.service')
            elif choice == '3':
                setup_hotspot()
            elif choice == '4':
                run('systemctl', 'stop', HOTSPOT_SERVICE)
            else:
                continue
        except (OSError, ValueError, subprocess.CalledProcessError):
            print('WLAN-Aktion fehlgeschlagen. Adapter und Eingaben prüfen.')
        input('Enter zurück zum WLAN-Menü …')


def bluetooth_menu():
    while True:
        print('\033[2J\033[HKanlo – Bluetooth')
        print('\n1  Status und gekoppelte Geräte anzeigen\n2  Bluetooth einschalten'
              '\n3  Bluetooth ausschalten\n4  Geräte koppeln und verwalten\n0  Zurück')
        choice = input('\nAuswahl: ').strip()
        if choice == '0':
            return
        try:
            if choice == '1':
                run('bluetoothctl', 'show', timeout=8)
                run('bluetoothctl', 'devices', 'Paired', timeout=8)
            elif choice == '2':
                run('rfkill', 'unblock', 'bluetooth')
                run('bluetoothctl', 'power', 'on', timeout=8)
            elif choice == '3':
                run('rfkill', 'block', 'bluetooth')
            elif choice == '4':
                print('Gerät in den Kopplungsmodus versetzen. Mit „quit“ zum Menü zurück.')
                print('Befehle: scan on · devices · pair ADRESSE · connect ADRESSE · remove ADRESSE')
                run('bluetoothctl', '--agent', 'KeyboardDisplay')
            else:
                continue
        except (OSError, subprocess.SubprocessError):
            print('Bluetooth-Aktion fehlgeschlagen. Adapter und Funkstatus prüfen.')
        input('Enter zurück zum Bluetooth-Menü …')


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
        print('\033[2J\033[HKanlo – Geräteeinstellungen')
        print_web_addresses()
        print('\n1  WLAN\n2  Bluetooth\n3  Anzeige-Diagnose\n4  Board-Anzeige neu starten'
              '\n5  Gerät einrichten\n6  Tastaturlayout\n0  Zurück zum Board')
        choice = input('\nAuswahl: ').strip()
        try:
            if choice == '1':
                wifi_menu()
                continue
            elif choice == '2':
                bluetooth_menu()
                continue
            elif choice == '3':
                run('fgconsole')
                for name, state, modes in display_connectors():
                    print(f'{name}: {state} ({", ".join(modes) or "keine Modi"})')
                run('journalctl', '-b', '-u', 'kanlo-tv.service', '-u', 'seatd.service', '-n', '25', '--no-pager')
            elif choice == '4':
                return_to_board(restart=True)
                continue
            elif choice == '5':
                setup_device()
            elif choice == '6':
                keyboard_menu()
            elif choice == '0':
                return_to_board()
                continue
            else:
                continue
        except (OSError, ValueError, subprocess.CalledProcessError):
            print('Einrichtung fehlgeschlagen. Eingaben und Geräte prüfen.')
        input('Enter zurück zum Menü …')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('command', choices=['firstboot', 'network', 'has-display', 'setup',
                                           'needs-welcome', 'welcome', 'hotspot-start', 'hotspot-stop'])
    command = parser.parse_args().command
    if command == 'has-display':
        raise SystemExit(0 if has_display() else 1)
    if command == 'needs-welcome':
        raise SystemExit(0 if needs_welcome() else 1)
    {'firstboot': firstboot, 'network': network, 'setup': setup,
     'welcome': welcome, 'hotspot-start': hotspot_serve, 'hotspot-stop': hotspot_stop}[command]()
