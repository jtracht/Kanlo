"""Measure an active Kanlo appliance without changing its data or settings."""
import argparse
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import time
import urllib.request

SERVICES = ('kanlo-server.service', 'kanlo-tv.service')
IMAGE = Path('/usr/share/kanlo/IMAGE.json')
FIRST_FRAME = Path('/run/kanlo-display/first-frame')


def image_identity():
    try:
        manifest = json.loads(IMAGE.read_text())
        return {field: manifest[field] for field in ('profile', 'version', 'source_commit')}
    except (OSError, ValueError, TypeError, KeyError):
        return None


def read_sysfs(path):
    try:
        return path.read_bytes().replace(b'\0', b'').decode(errors='replace').strip()
    except OSError:
        return None


def display_state(drm=Path('/sys/class/drm')):
    connectors = {}
    for status in sorted(drm.glob('card*-*/status')):
        connector = status.parent
        modes = read_sysfs(connector / 'modes') or ''
        connectors[connector.name] = {
            'status': read_sysfs(status),
            'enabled': read_sysfs(connector / 'enabled'),
            'advertised_modes': modes.splitlines(),
        }
    return {
        'foreground_console': read_sysfs(Path('/sys/class/tty/tty0/active')),
        'renderer_first_frame': read_sysfs(FIRST_FRAME),
        'connectors': connectors,
    }


def service_state(unit):
    pid = int(subprocess.check_output(
        ['systemctl', 'show', '--property=MainPID', '--value', unit], text=True))
    if not pid:
        return None
    group = subprocess.check_output(
        ['systemctl', 'show', '--property=ControlGroup', '--value', unit], text=True).strip()
    if not group.startswith('/'):
        return None
    cgroup = Path('/sys/fs/cgroup') / group.lstrip('/')
    cpu = dict(line.split() for line in (cgroup / 'cpu.stat').read_text().splitlines())
    memory = {'rss_kib': 0, 'pss_kib': 0}
    for member in (cgroup / 'cgroup.procs').read_text().split():
        try:
            lines = (Path('/proc') / member / 'smaps_rollup').read_text().splitlines()
        except (FileNotFoundError, ProcessLookupError):
            continue
        for line in lines:
            name, _, value = line.partition(':')
            key = name.lower() + '_kib'
            if key in memory:
                memory[key] += int(value.split()[0])
    return {'pid': pid, 'cpu_microseconds': int(cpu['usage_usec']), **memory}


def summarize(values):
    ordered = sorted(values)
    return {'median': round(statistics.median(ordered), 2),
            'p95': round(ordered[(95 * len(ordered) + 99) // 100 - 1], 2),
            'max': round(ordered[-1], 2)}


def read_api(path):
    with urllib.request.urlopen('http://127.0.0.1/api/v1/' + path, timeout=2) as response:
        return json.load(response)


def workload(name):
    board = next((item for item in read_api('boards') if item['name'] == name), None)
    if board is None:
        raise ValueError(f'Test board {name!r} not found')
    tickets = read_api(f"boards/{board['id']}/snapshot")['workspace']['tickets']
    return {'board': name, 'tickets': len(tickets)}


def main(seconds, board=None):
    fixture = workload(board) if board else None
    start = time.monotonic()
    display_at_start = display_state()
    latency, errors = [], 0
    samples = {service: [] for service in SERVICES}
    version = None
    overlay_modes = set()
    display_api_failures = 0
    while time.monotonic() - start < seconds:
        tick = time.monotonic()
        try:
            version = read_api('health')['version']
            latency.append((time.monotonic() - tick) * 1000)
        except (OSError, KeyError, ValueError):
            errors += 1
        if fixture:
            try:
                state = read_api('display')
                overlays = state['overlays']
                overlay_modes.add(overlays[-1]['kind'] if overlays else 'board')
                if state.get('companion_ticket'):
                    overlay_modes.add('companion_ticket')
            except (OSError, KeyError, ValueError):
                display_api_failures += 1
        for service, records in samples.items():
            try:
                state = service_state(service)
                if state:
                    records.append((time.monotonic(), state))
            except (FileNotFoundError, ProcessLookupError):
                continue
        time.sleep(max(0, min(start + seconds, tick + 1) - time.monotonic()))
    processes = {}
    for service, records in samples.items():
        if not records:
            processes[service] = {'samples': 0}
            continue
        intervals = [(last['cpu_microseconds'] - first['cpu_microseconds']) / 10_000 / (end - begin)
                     for (begin, first), (end, last) in zip(records, records[1:])
                     if first['pid'] == last['pid'] and end > begin]
        processes[service] = {
            'samples': len(records),
            'pid_changes': sum(first['pid'] != last['pid']
                               for (_, first), (_, last) in zip(records, records[1:])),
            'cpu_percent_one_core': summarize(intervals) if intervals else None,
            'rss_kib': summarize([state['rss_kib'] for _, state in records]),
            'pss_kib': summarize([state['pss_kib'] for _, state in records]),
        }
    print(json.dumps({
        'device_model': read_sysfs(Path('/proc/device-tree/model')) or
                        read_sysfs(Path('/sys/class/dmi/id/product_name')),
        'kernel_machine': platform.machine(),
        'userspace_arch': subprocess.check_output(['dpkg', '--print-architecture'], text=True).strip(),
        'image': image_identity(),
        'workload': fixture,
        'kanlo_version': version,
        'duration_seconds': seconds,
        'elapsed_seconds': round(time.monotonic() - start, 2),
        'health_response_ms': summarize(latency) if latency else None,
        'health_failures': errors,
        'observed_display_modes': sorted(overlay_modes) if fixture else None,
        'display_api_failures': display_api_failures if fixture else None,
        'display_at_start': display_at_start,
        'display_at_end': display_state(),
        'services': processes,
    }, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--seconds', type=int, default=600)
    parser.add_argument('--board', help='Exact name of the imported test board')
    args = parser.parse_args()
    if os.geteuid() != 0 or args.seconds < 10:
        parser.error('Run with sudo and select at least ten seconds')
    main(args.seconds, args.board)
