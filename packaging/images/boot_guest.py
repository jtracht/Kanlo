"""Injected into an isolated test image only; never installed in shipped images."""
import json
import io
import os
from pathlib import Path
import queue
import shutil
import stat
import subprocess
import tarfile
import tempfile
import threading
import time
import urllib.request
import uuid

import display
import tv_markdown
import update


def get(path, origin='http://127.0.0.1:8080'):
    with urllib.request.urlopen(origin + path, timeout=5) as response:
        return json.load(response)


def verify_update_rollback():
    before = get('/api/v1/health')
    architecture = subprocess.check_output(['dpkg', '--print-architecture'], text=True).strip()
    root = f"kanlo-{before['version']}-linux-{architecture}"
    build = {'schema_version': 1, 'version': before['version'], 'arch': architecture,
             'minimum_os': 'Debian 13', 'binary_sha256': update.checksum(update.BINARY)}
    with tempfile.TemporaryDirectory() as directory:
        package = Path(directory) / 'same-version-fixture.tar.gz'
        with tarfile.open(package, 'w:gz', compresslevel=1) as archive:
            manifest = json.dumps(build).encode()
            member = tarfile.TarInfo(f'{root}/BUILD.json')
            member.size = len(manifest)
            archive.addfile(member, io.BytesIO(manifest))
            archive.add(update.BINARY, arcname=f'{root}/bin/kanlo-server')
        updater = update.Updater(update.BINARY, update.DATABASE, update.STATE, update.System())
        updater.apply(package, update.checksum(package), architecture)
        assert get('/api/v1/health')['epoch'] == before['epoch']
        updater.rollback()
    after = get('/api/v1/health')
    assert after['epoch'] != before['epoch'] and after['instance_id'] == before['instance_id']


def mutate(command):
    health = get('/api/v1/health')
    body = {'op_id': str(uuid.uuid4()), 'client_id': 'image-boot-test', 'server_instance_id': health['instance_id'], 'base_epoch': health['epoch'], 'base_context': None, 'command': command}
    request = urllib.request.Request('http://127.0.0.1:8080/api/v1/commands', data=json.dumps(body).encode(), headers={'Content-Type': 'application/json', 'Origin': 'http://127.0.0.1:8080'})
    with urllib.request.urlopen(request, timeout=5) as response:
        return json.load(response)['result']['entity_id']


def service_pid(unit):
    return int(subprocess.check_output(
        ['systemctl', 'show', '--property=MainPID', '--value', unit], text=True
    ))


def wait_for_new_pid(unit, old):
    for _ in range(25):
        pid = service_pid(unit)
        if pid and pid != old:
            return pid
        time.sleep(1)
    raise RuntimeError(f'{unit} did not start again after process termination')


def verify_independent_restarts(board_id):
    tv = 'kanlo-tv.service'
    server = 'kanlo-server.service'
    # A QEMU fixture has no physical connector. Open the normal display service
    # on its console after the headless checks, using its console fallback.
    override = Path('/run/systemd/system/kanlo-tv.service.d/image-test.conf')
    override.parent.mkdir(parents=True, exist_ok=True)
    override.write_text('[Service]\nExecCondition=\n')
    subprocess.run(['systemctl', 'daemon-reload'], check=True)
    stop = threading.Event()
    updates = queue.Queue(maxsize=1)
    display_updates = queue.Queue(maxsize=1)
    threading.Thread(target=display.reader, args=(updates, stop, display_updates), daemon=True).start()

    def wait_for_display(ticket=None):
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            boards, online = updates.get(timeout=max(0.1, deadline - time.monotonic()))
            if online and (ticket is None or any(
                item['id'] == ticket for board in boards for item in board['tickets']
            )):
                return
        raise RuntimeError('Display did not receive the expected board snapshot')

    def create_display_ticket():
        return mutate({'kind': 'create_ticket', 'board_id': board_id, 'title': 'Live display probe',
                       'description': '', 'importance': 0, 'column_id': None, 'parent_id': None, 'type_id': None})

    def show_ticket(ticket, minutes=None):
        origin = 'http://127.0.0.1:8080'
        req = urllib.request.Request(origin + '/api/v1/display',
            data=json.dumps({'kind': 'show_ticket', 'ticket_id': ticket, 'minutes': minutes}).encode(),
            headers={'Content-Type': 'application/json', 'Origin': origin})
        with urllib.request.urlopen(req, timeout=5) as response:
            assert response.status == 200
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            state = display_updates.get(timeout=max(0.1, deadline - time.monotonic()))
            if state['overlays'] and state['overlays'][-1].get('ticket_id') == ticket:
                return state
        raise RuntimeError('Display did not receive the overlay action')

    try:
        wait_for_display()
        ticket = create_display_ticket()
        wait_for_display(ticket)
        show_ticket(ticket)
        server_before = service_pid(server)
        subprocess.run(['systemctl', 'start', tv], check=True)
        display_before = wait_for_new_pid(tv, 0)
        assert service_pid(server) == server_before
        subprocess.run(['systemctl', 'kill', '--kill-whom=main', '--signal=KILL', tv], check=True)
        display_after = wait_for_new_pid(tv, display_before)
        assert service_pid(server) == server_before
        subprocess.run(['systemctl', 'kill', '--kill-whom=main', '--signal=KILL', server], check=True)
        wait_for_new_pid(server, server_before)
        assert service_pid(tv) == display_after
        previous = ticket
        for _ in range(15):
            try:
                ticket = create_display_ticket()
                break
            except OSError:
                time.sleep(1)
        else:
            raise RuntimeError('Server did not accept writes after restarting')
        wait_for_display(ticket)
        pair = show_ticket(ticket, minutes=0)
        assert pair['companion_ticket']['ticket_id'] == previous
        assert pair['expires_at'] is None
    finally:
        stop.set()
        subprocess.run(['systemctl', 'stop', tv], check=True)
        override.unlink()
        subprocess.run(['systemctl', 'daemon-reload'], check=True)


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
    assert provisioned['hostname'] == 'kanlo-image-test'
    assert Path('/etc/hostname').read_text().strip() == 'kanlo-image-test'
    assert not provisioned['ssh_enabled']
    assert not Path('/etc/ssh/ssh_host_ed25519_key').exists()
    assert not Path('/boot/firmware/KANLO_CONFIG.json').exists()
    mode = stat.S_IMODE(Path('/var/lib/kanlo').stat().st_mode)
    assert mode == 0o700
    assert Path('/etc/cloud/cloud-init.disabled').is_file()
    assert Path('/usr/sbin/wpa_supplicant').is_file()
    assert os.access('/usr/bin/nmtui-connect', os.X_OK)
    assert os.access('/usr/bin/nmtui-edit', os.X_OK)
    assert shutil.which('bluetoothctl') and shutil.which('rfkill') and shutil.which('chvt')
    assert Path('/etc/systemd/system/kanlo-welcome.service').is_file()
    assert not Path('/var/lib/kanlo-appliance/welcome-complete').exists()
    assert os.access('/usr/sbin/dnsmasq', os.X_OK)
    assert Path('/etc/systemd/system/kanlo-hotspot.service').is_file()
    assert Path('/usr/lib/kanlo/update.py').is_file()
    assert ('table', 'row', None) in tv_markdown.markdown_rows('| Header |\n|---|\n| row |')
    subprocess.run(['/usr/bin/python3', '-c',
                    'import qrcode; assert qrcode.QRCode(border=4).get_matrix()'], check=True)
    assert Path('/lib/firmware/regulatory.db').is_file()
    wifi = Path('/etc/NetworkManager/system-connections/kanlo-wifi.nmconnection')
    assert wifi.is_file() and stat.S_IMODE(wifi.stat().st_mode) == 0o600
    assert 'noatime' in subprocess.check_output(['findmnt', '-n', '-o', 'OPTIONS', '/'], text=True).strip().split(',')
    swaps = [line.split() for line in Path('/proc/swaps').read_text().splitlines()[1:]]
    assert swaps and all(swap[0].startswith('/dev/zram') and 0 < int(swap[2]) <= 512 * 1024 for swap in swaps)
    for path in ['/etc', '/etc/systemd/system/kanlo-server.service', '/usr/lib/kanlo/appliance.py', '/usr/lib/kanlo/display.py']:
        assert stat.S_IMODE(Path(path).stat().st_mode) & 0o022 == 0
    assert os.statvfs('/').f_blocks * os.statvfs('/').f_frsize > 10 * 1024**3
    assert subprocess.run(['systemctl', 'is-active', '--quiet', 'ssh.service']).returncode != 0
    assert subprocess.run(['systemctl', 'is-active', '--quiet', 'kanlo-tv.service']).returncode != 0
    assert subprocess.run(['systemctl', 'is-active', '--quiet', 'kanlo-setup.service']).returncode == 0
    assert subprocess.run(['systemctl', 'is-active', '--quiet', 'kanlo-hotspot.service']).returncode != 0
    assert subprocess.run(['systemctl', 'is-active', '--quiet', 'NetworkManager.service']).returncode == 0
    assert subprocess.run(['systemctl', 'is-active', '--quiet', 'seatd.service']).returncode == 0
    assert 'display.py --launch' in subprocess.check_output(
        ['systemctl', 'show', '--property=ExecStart', '--value', 'kanlo-tv.service'], text=True
    )
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
        board = before['board_id']
    else:
        board = mutate({'kind': 'create_board', 'name': 'Image acceptance', 'prefix': 'IMGTEST'})
        ticket = mutate({'kind': 'create_ticket', 'board_id': board, 'title': 'Survives an image reboot', 'description': '', 'importance': 100, 'column_id': None, 'parent_id': None, 'type_id': None})
        marker.write_text(json.dumps({'machine_id': identity, 'instance_id': state['instance_id'], 'configured_at': provisioned['configured_at'], 'board_id': board, 'ticket_id': ticket}))
        phase = 'firstboot'
    verify_independent_restarts(board)
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
    if phase == 'reboot':
        verify_update_rollback()
        assert get('/api/v1/tickets/' + before['ticket_id'])['title'] == 'Survives an image reboot'
    hotspot_verified = False
    if phase == 'firstboot' and Path(__file__).with_name('boot_wifi.py').is_file():
        from boot_wifi import verify_hotspot
        verify_hotspot(state['instance_id'])
        hotspot_verified = True
    subprocess.run(['systemctl', 'stop', 'kanlo-server.service'], check=True)
    checked = subprocess.run(['runuser', '-u', 'kanlo', '--', 'env', 'KANLO_DATA_DIR=/var/lib/kanlo', '/usr/local/lib/kanlo/kanlo-server', '--check'], check=True, capture_output=True, text=True)
    assert checked.stdout.strip() == 'ok'
    print('KANLO_IMAGE_BOOT_OK ' + json.dumps({'phase': phase, 'machine_id': identity, 'instance_id': state['instance_id'], 'kernel_machine': os.uname().machine, 'kernel_release': os.uname().release, 'userspace_arch': subprocess.check_output(['dpkg', '--print-architecture'], text=True).strip(), 'root_expanded': True, 'sqlite_integrity': 'ok', 'lan_http_verified': True, 'ram_swap_verified': True, 'independent_service_restarts': True, 'live_display_verified': True, 'update_rollback_verified': phase == 'reboot', 'wifi_hotspot_verified': hotspot_verified}), flush=True)


try:
    main()
except Exception as error:
    print('KANLO_IMAGE_BOOT_FAILED ' + repr(error), flush=True)
    raise
finally:
    subprocess.run(['systemctl', 'poweroff'], check=False)
