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


def main(seconds):
    start = time.monotonic()
    latency, errors = [], 0
    samples = {service: [] for service in SERVICES}
    version = None
    while time.monotonic() - start < seconds:
        tick = time.monotonic()
        try:
            with urllib.request.urlopen('http://127.0.0.1:8080/api/v1/health', timeout=2) as response:
                version = json.load(response)['version']
            latency.append((time.monotonic() - tick) * 1000)
        except (OSError, KeyError, ValueError):
            errors += 1
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
        'kernel_machine': platform.machine(),
        'userspace_arch': subprocess.check_output(['dpkg', '--print-architecture'], text=True).strip(),
        'kanlo_version': version,
        'duration_seconds': seconds,
        'health_response_ms': summarize(latency) if latency else None,
        'health_failures': errors,
        'services': processes,
    }, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--seconds', type=int, default=600)
    args = parser.parse_args()
    if os.geteuid() != 0 or args.seconds < 10:
        parser.error('Run with sudo and select at least ten seconds')
    main(args.seconds)
