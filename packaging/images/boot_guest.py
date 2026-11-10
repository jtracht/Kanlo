"""Injected into an isolated test image only; never installed in shipped images."""
import json
import os
from pathlib import Path
import stat
import subprocess
import time
import urllib.request
import uuid


def get(path, origin='http://127.0.0.1:8080'):
    with urllib.request.urlopen(origin + path, timeout=5) as response:
        return json.load(response)


def mutate(command):
    health = get('/api/v1/health')
    body = {'op_id': str(uuid.uuid4()), 'client_id': 'image-boot-test', 'server_instance_id': health['instance_id'], 'base_epoch': health['epoch'], 'base_context': None, 'command': command}
    request = urllib.request.Request('http://127.0.0.1:8080/api/v1/commands', data=json.dumps(body).encode(), headers={'Content-Type': 'application/json', 'Origin': 'http://127.0.0.1:8080'})
    with urllib.request.urlopen(request, timeout=5) as response:
        return json.load(response)['result']['entity_id']


def main():
    for _ in range(90):
        try:
            state = get('/api/v1/bootstrap')
            break
        except (OSError, ValueError):
            time.sleep(2)
    else:
        raise RuntimeError('Kanlo did not start')
    assert state['boards']
    identity = Path('/etc/machine-id').read_text().strip()
    assert len(identity) == 32
    provisioned = json.loads(Path('/var/lib/kanlo-appliance/provisioned.json').read_text())
    assert provisioned['hostname'].startswith('kanlo-')
    assert not provisioned['ssh_enabled']
    assert not Path('/etc/ssh/ssh_host_ed25519_key').exists()
    assert not Path('/boot/firmware/KANLO_CONFIG.json').exists()
    mode = stat.S_IMODE(Path('/var/lib/kanlo').stat().st_mode)
    assert mode == 0o700
    assert 'noatime' in subprocess.check_output(['findmnt', '-n', '-o', 'OPTIONS', '/'], text=True).strip().split(',')
    swaps = json.loads(subprocess.check_output(['swapon', '--show', '--json', '--bytes'], text=True))['swapdevices']
    assert swaps and all(swap['name'].startswith('/dev/zram') and 0 < swap['size'] <= 512 * 1024**2 for swap in swaps)
    for path in ['/etc', '/etc/systemd/system/kanlo-server.service', '/usr/lib/kanlo/appliance.py', '/usr/lib/kanlo/display.py']:
        assert stat.S_IMODE(Path(path).stat().st_mode) & 0o022 == 0
    assert os.statvfs('/').f_blocks * os.statvfs('/').f_frsize > 10 * 1024**3
    assert subprocess.run(['systemctl', 'is-active', '--quiet', 'ssh.service']).returncode != 0
    assert subprocess.run(['systemctl', 'is-active', '--quiet', 'kanlo-tv.service']).returncode != 0
    assert subprocess.run(['systemctl', 'is-active', '--quiet', 'kanlo-setup.service']).returncode == 0
    assert subprocess.run(['systemctl', 'is-active', '--quiet', 'NetworkManager.service']).returncode == 0
    assert subprocess.run(['systemctl', 'is-active', '--quiet', 'seatd.service']).returncode == 0
    lan_verified = False
    for _ in range(60):
        interfaces = json.loads(subprocess.check_output(['ip', '-j', '-4', 'address', 'show'], text=True))
        addresses = [address['local'] for interface in interfaces if interface['ifname'] != 'lo' for address in interface.get('addr_info', []) if address.get('family') == 'inet']
        for address in addresses:
            try:
                if get('/api/v1/bootstrap', 'http://' + address + ':8080')['instance_id'] == state['instance_id']:
                    lan_verified = True
                    break
            except OSError:
                pass
        if lan_verified:
            break
        time.sleep(1)
    assert lan_verified, 'DHCP LAN address must reach the real Kanlo listener'
    marker = Path('/var/lib/kanlo-appliance/test-identity.json')
    if marker.exists():
        before = json.loads(marker.read_text())
        assert before['machine_id'] == identity
        assert before['instance_id'] == state['instance_id']
        assert before['configured_at'] == provisioned['configured_at']
        snapshot = get('/api/v1/boards/' + before['board_id'] + '/snapshot')
        assert any(ticket['id'] == before['ticket_id'] and ticket['title'] == 'Survives an image reboot' for ticket in snapshot['workspace']['tickets'])
        phase = 'reboot'
    else:
        board = mutate({'kind': 'create_board', 'name': 'Image acceptance', 'prefix': 'IMGTEST'})
        ticket = mutate({'kind': 'create_ticket', 'board_id': board, 'title': 'Survives an image reboot', 'description': '', 'importance': 100, 'column_id': None, 'parent_id': None, 'type_id': None})
        marker.write_text(json.dumps({'machine_id': identity, 'instance_id': state['instance_id'], 'configured_at': provisioned['configured_at'], 'board_id': board, 'ticket_id': ticket}))
        phase = 'firstboot'
    # Actually restart the service and check the same SQLite instance remains intact.
    subprocess.run(['systemctl', 'restart', 'kanlo-server.service'], check=True)
    for _ in range(30):
        try:
            assert get('/api/v1/bootstrap')['instance_id'] == state['instance_id']
            break
        except OSError:
            time.sleep(1)
    else:
        raise RuntimeError('Restart failed')
    subprocess.run(['systemctl', 'stop', 'kanlo-server.service'], check=True)
    checked = subprocess.run(['runuser', '-u', 'kanlo', '--', 'env', 'KANLO_DATA_DIR=/var/lib/kanlo', '/usr/local/lib/kanlo/kanlo-server', '--check'], check=True, capture_output=True, text=True)
    assert checked.stdout.strip() == 'ok'
    print('KANLO_IMAGE_BOOT_OK ' + json.dumps({'phase': phase, 'machine_id': identity, 'instance_id': state['instance_id'], 'root_expanded': True, 'sqlite_integrity': 'ok', 'lan_http_verified': True, 'ram_swap_verified': True}), flush=True)


try:
    main()
except Exception as error:
    print('KANLO_IMAGE_BOOT_FAILED ' + repr(error), flush=True)
    raise
finally:
    subprocess.run(['systemctl', 'poweroff'], check=False)
