"""Exercise the appliance hotspot with a separate Linux Wi-Fi client."""
import json
from pathlib import Path
import subprocess
import tempfile
import time

UNIT = 'kanlo-hotspot.service'
CLIENT = 'kanlo-wifi-client'
GUEST = ('ip', 'netns', 'exec', CLIENT)
PROFILE = Path('/run/NetworkManager/system-connections/kanlo-setup.nmconnection')


def command(*args, check=True):
    return subprocess.run(args, check=check, text=True, stdout=subprocess.PIPE,
                          timeout=55).stdout.strip()


def wait_until(predicate, message, seconds=20):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.5)
    raise RuntimeError(message)


def device_rows():
    return [line.split(':', 2) for line in command(
        'nmcli', '-t', '-f', 'DEVICE,TYPE,STATE', 'device', 'status').splitlines()]


def verify_hotspot(instance):
    command('modprobe', 'mac80211_hwsim', 'radios=2')
    radios = sorted(Path('/sys/class/ieee80211').glob('phy*'))
    assert len(radios) == 2
    station, = (radios[1] / 'device/net').iterdir()
    command('ip', 'netns', 'add', CLIENT)
    override = Path('/run/systemd/system') / (UNIT + '.d') / 'acceptance.conf'
    override.parent.mkdir(parents=True, exist_ok=True)
    override.write_text('[Service]\nRuntimeMaxSec=30s\n')
    supplicant = None
    try:
        command('iw', 'phy', radios[1].name, 'set', 'netns', 'name', CLIENT)
        command(*GUEST, 'ip', 'link', 'set', station.name, 'up')
        for interface, kind, state in device_rows():
            if kind == 'ethernet' and state.startswith('connected'):
                command('nmcli', 'device', 'disconnect', interface)
        wait_until(lambda: any(kind == 'wifi' and state == 'disconnected'
                               for _, kind, state in device_rows()), 'No virtual Wi-Fi adapter ready')
        command('systemctl', 'daemon-reload')
        command('systemctl', 'start', UNIT)
        credentials = json.loads(Path('/run/kanlo-hotspot/credentials.json').read_text())
        with tempfile.TemporaryDirectory() as folder:
            config = Path(folder) / 'wpa.conf'
            config.write_text('network={\n'
                              'ssid=' + json.dumps(credentials['ssid']) + '\n'
                              'psk=' + json.dumps(credentials['password']) + '\n'
                              'key_mgmt=WPA-PSK\nproto=RSN\n}\n')
            config.chmod(0o600)
            supplicant = subprocess.Popen((*GUEST, 'wpa_supplicant', '-Dnl80211', '-i',
                                           station.name, '-c', str(config)),
                                          stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            wait_until(lambda: 'Connected to' in command(*GUEST, 'iw', 'dev', station.name, 'link'),
                       'WPA2 client could not associate', seconds=12)
            command(*GUEST, 'dhcpcd', '-4', '-1', '-w', '-t', '12', '-L',
                    '-C', 'resolv.conf', '-C', 'hostname', station.name)
            addresses = json.loads(command(*GUEST, 'ip', '-j', '-4', 'addr', 'show', 'dev', station.name))
            assert any(info['local'].startswith('10.42.0.') for info in addresses[0]['addr_info'])
            origin = 'http://10.42.0.1'
            request = (*GUEST, 'curl', '-fsS', '--retry', '3', '--retry-connrefused', '--max-time', '3')
            assert json.loads(command(*request, origin + '/api/v1/bootstrap'))['instance_id'] == instance
            assert '<title>Kanlo</title>' in command(*request, origin + '/')
            wait_until(lambda: command('systemctl', 'show', '-p', 'Result', '--value', UNIT) == 'timeout'
                       and not PROFILE.exists(), 'Hotspot did not stop and remove its profile', seconds=40)
            wait_until(lambda: 'Not connected' in command(*GUEST, 'iw', 'dev', station.name, 'link'),
                       'Wireless client remained connected after hotspot timeout')
    finally:
        if supplicant is not None:
            supplicant.terminate()
            supplicant.wait(timeout=5)
        command('systemctl', 'stop', UNIT, check=False)
        override.unlink(missing_ok=True)
        command('ip', 'netns', 'delete', CLIENT, check=False)
