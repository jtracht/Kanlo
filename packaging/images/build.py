"""Build only owned regular disk-image files, never a physical block device."""
import argparse
import contextlib
import hashlib
import json
import lzma
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
from release_package import git, inspect, version

PROFILES = json.loads((ROOT / 'packaging/images/bases.json').read_text())['profiles']


def raspberry_pi_cmdline(original):
    excluded = ('init=', 'systemd.run=', 'systemd.run_success_action=',
                'systemd.run_failure_action=', 'loglevel=', 'systemd.show_status=')
    args = [value for value in original.split() if not value.startswith(excluded)
            and not re.fullmatch(r'console=tty\d+(?:,.*)?', value)]
    return ' '.join([*args, 'console=tty3', 'loglevel=3', 'systemd.show_status=no']) + '\n'


@contextlib.contextmanager
def workspace(profile, work):
    directory = Path(tempfile.mkdtemp(prefix='kanlo-' + profile + '-', dir=work))
    try:
        yield directory
    except BaseException:
        print('Failed image and boot logs retained for diagnosis:', directory, flush=True)
        raise
    else:
        shutil.rmtree(directory)


def run(*args, **kwargs):
    return subprocess.run([str(a) for a in args], check=True, text=True, **kwargs)


def checksum(path, algorithm='sha256'):
    digest = hashlib.new(algorithm)
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(4 * 1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def download(profile, cache):
    base = PROFILES[profile]
    url = base['url']
    if not url.startswith(('https://downloads.raspberrypi.com/', 'https://cloud.debian.org/')):
        raise ValueError('Untrusted base-image source')
    cache.mkdir(parents=True, exist_ok=True)
    path = cache / url.rsplit('/', 1)[1]
    if path.is_symlink():
        raise ValueError('Cached base must be a regular file')
    if not path.exists():
        temporary = path.with_suffix(path.suffix + '.download')
        with urllib.request.urlopen(url, timeout=120) as response, temporary.open('wb') as stream:
            shutil.copyfileobj(response, stream, 4 * 1024 * 1024)
        temporary.replace(path)
    if checksum(path, base['hash_algorithm']) != base['checksum']:
        raise ValueError('Official base-image checksum mismatch; update pins explicitly')
    return path


@contextlib.contextmanager
def mounted(image, work, expand=False):
    image = image.resolve()
    if image.is_symlink() or not image.is_file() or not image.is_relative_to(work.resolve()):
        raise ValueError('Only a regular image inside the explicit work directory is allowed')
    loop = run('losetup', '--find', '--show', '--partscan', image, capture_output=True).stdout.strip()
    if not loop.startswith('/dev/loop'):
        raise ValueError('Expected an allocated loop device')
    root = work / 'root'; root.mkdir(exist_ok=True)
    try:
        run('udevadm', 'settle') if shutil.which('udevadm') else None
        parts = json.loads(run('lsblk', '--json', '-p', '-o', 'NAME,FSTYPE,UUID,PARTUUID', loop, capture_output=True).stdout)['blockdevices'][0].get('children', [])
        for part in parts:
            device = Path(part['name'])
            if not device.exists():
                numbers = (Path('/sys/class/block') / device.name / 'dev').read_text().strip().split(':')
                os.mknod(device, 0o60600, os.makedev(*map(int, numbers)))
            # Containers often have no udev database; probe the owned partition directly.
            probe = subprocess.run(['blkid', '-p', '-o', 'export', part['name']], capture_output=True, text=True)
            fields = dict(line.split('=', 1) for line in probe.stdout.splitlines() if '=' in line)
            part.update(fstype=fields.get('TYPE'), uuid=fields.get('UUID'), partuuid=fields.get('PART_ENTRY_UUID'))
        root_part = None
        for part in parts:
            if part['fstype'] == 'ext4':
                run('mount', part['name'], root)
                if (root / 'etc/os-release').exists():
                    root_part = part
                    break
                run('umount', root)
        if root_part is None:
            raise ValueError('No Debian root partition')
        if expand:
            run('umount', root)
            number = (Path('/sys/class/block') / Path(root_part['name']).name / 'partition').read_text().strip()
            run('parted', '--script', '--fix', loop, 'resizepart', number, '100%')
            checked = subprocess.run(['e2fsck', '-pf', root_part['name']])
            if checked.returncode not in (0, 1):
                raise ValueError('Base root filesystem check failed')
            run('resize2fs', root_part['name'])
            run('mount', root_part['name'], root)
        # Evaluate vendor fstab UUIDs only against partitions on this owned loop.
        for line in (root / 'etc/fstab').read_text().splitlines():
            fields = line.split()
            if not fields or fields[0].startswith('#') or len(fields) < 3 or not fields[1].startswith('/boot'):
                continue
            target = root / fields[1].lstrip('/')
            if not target.resolve().is_relative_to(root.resolve()):
                raise ValueError('Unsafe fstab mount')
            match = next((p for p in parts if fields[0] in ('UUID=' + str(p['uuid']), 'PARTUUID=' + str(p['partuuid']), p['name'])), None)
            if match is None:
                raise ValueError('Boot partition cannot be resolved on the image')
            target.mkdir(parents=True, exist_ok=True)
            run('mount', match['name'], target)
        yield root, root_part, parts
    finally:
        run('sync')
        subprocess.run(['umount', '-R', str(root)], check=False)
        run('losetup', '-d', loop)


@contextlib.contextmanager
def chroot_mounts(root):
    try:
        for source in ['/dev', '/proc', '/sys']:
            target = root / source.lstrip('/')
            target.mkdir(exist_ok=True)
            run('mount', '--rbind', source, target)
            run('mount', '--make-rslave', target)
        yield
    finally:
        for source in ['/sys', '/proc', '/dev']:
            run('umount', '-R', root / source.lstrip('/'))


def install_overlay(root):
    # Do not copy Windows directory modes onto the guest's /etc or root.
    overlay = ROOT / 'packaging/images/overlay'
    for source_path in sorted(overlay.rglob('*')):
        target_path = root / source_path.relative_to(overlay)
        if source_path.is_dir():
            target_path.mkdir(parents=True, exist_ok=True)
            target_path.chmod(0o755)
        else:
            shutil.copyfile(source_path, target_path)
            target_path.chmod(0o644)
    shutil.copy2(ROOT / 'packaging/systemd/kanlo-server.service', root / 'etc/systemd/system/kanlo-server.service')
    (root / 'etc/systemd/system/kanlo-server.service').chmod(0o644)
    (root / 'usr/lib/kanlo').mkdir(parents=True, exist_ok=True)
    for name in ['appliance.py', 'display.py', 'tv_markdown.py', 'customize.sh', 'update.py']:
        shutil.copy2(ROOT / 'packaging/images' / name, root / 'usr/lib/kanlo' / name)
        (root / 'usr/lib/kanlo' / name).chmod(0o644)
    os.chmod(root / 'etc/NetworkManager/dispatcher.d/90-kanlo', 0o755)


def build(profile, package, cache, work, output):
    commit = git('rev-parse', 'HEAD')
    if git('status', '--porcelain', '--untracked-files=no'):
        raise ValueError('Commit source changes before building an image')
    base = PROFILES[profile]
    metadata = inspect(package)
    if metadata['arch'] != base['arch'] or metadata['source_commit'] != git('rev-parse', 'HEAD'):
        raise ValueError('Image requires the matching package from this exact commit')
    work.mkdir(parents=True, exist_ok=True); output.mkdir(parents=True, exist_ok=True)
    with workspace(profile, work) as directory:
        directory = Path(directory)
        image = directory / 'disk.img'
        source = download(profile, cache)
        if base['format'] == 'xz':
            with lzma.open(source) as stream, image.open('wb') as target:
                shutil.copyfileobj(stream, target, 4 * 1024 * 1024)
        else:
            run('qemu-img', 'convert', '-f', 'qcow2', '-O', 'raw', source, image)
        # A power-of-two SD capacity is also required by QEMU's Raspberry Pi model.
        run('truncate', '-s', '8G', image)
        with mounted(image, directory, expand=True) as (root, root_part, parts):
            if not re.search(r'^VERSION_ID="?13"?$', (root / 'etc/os-release').read_text(), re.M):
                raise ValueError('Only Debian/Raspberry Pi OS 13 is supported')
            import tarfile
            with tarfile.open(package) as tar:
                tar.extractall(directory / 'package', filter='data')
            extracted = directory / 'package' / f'kanlo-{version()}-linux-{base["arch"]}'
            (root / 'usr/local/lib/kanlo').mkdir(parents=True, exist_ok=True)
            shutil.copy2(extracted / 'bin/kanlo-server', root / 'usr/local/lib/kanlo/kanlo-server')
            shutil.copytree(extracted, root / 'usr/share/kanlo', dirs_exist_ok=True)
            # Keep exactly one installed server binary, not a second distribution copy.
            shutil.rmtree(root / 'usr/share/kanlo/bin')
            install_overlay(root)
            # Chroot needs a real resolver until NetworkManager starts on the device.
            (root / 'etc/resolv.conf').unlink(missing_ok=True)
            shutil.copyfile('/etc/resolv.conf', root / 'etc/resolv.conf')
            # Empty unused data sectors are compressed, while ext4 itself expands on-device.
            with chroot_mounts(root):
                run('chroot', root, '/usr/bin/env', 'KANLO_IMAGE_FAMILY=' + base['family'], '/bin/sh', '/usr/lib/kanlo/customize.sh')
            # Raspberry Pi uses its original firmware, DTBs, kernel and auto-initramfs.
            boot = root / 'boot/firmware'
            if base['family'] == 'rpi':
                cmdline = boot / 'cmdline.txt'
                cmdline.write_text(raspberry_pi_cmdline(cmdline.read_text()))
            else:
                # FAT provisioning area: same KANLO_CONFIG.json path as Raspberry Pi.
                efi = root / 'boot/efi'
                (root / 'etc/systemd/system/boot-firmware.mount').write_text('[Unit]\nBefore=kanlo-firstboot.service\nAfter=boot-efi.mount\nRequires=boot-efi.mount\n[Mount]\nWhat=/boot/efi\nWhere=/boot/firmware\nType=none\nOptions=bind\n[Install]\nWantedBy=local-fs.target\n')
                wants = root / 'etc/systemd/system/local-fs.target.wants'
                wants.mkdir(exist_ok=True)
                (wants / 'boot-firmware.mount').symlink_to('../boot-firmware.mount')
                if not efi.is_mount():
                    raise ValueError('Generic image must have a mounted UEFI system partition')
            provision = boot if base['family'] == 'rpi' else root / 'boot/efi'
            shutil.copy2(ROOT / 'packaging/images/config.example.json', provision / 'KANLO_CONFIG.example.json')
            report = {'schema_version': 1, 'profile': profile, 'version': version(), 'source_commit': metadata['source_commit'], 'base': base, 'server': metadata, 'boot_test': None, 'physical_device_certified': False, 'packages': (root / 'usr/share/kanlo/image-packages.tsv').read_text().splitlines()}
            (root / 'usr/share/kanlo/IMAGE.json').write_text(json.dumps({k: v for k, v in report.items() if k != 'packages'}, indent=2) + '\n')
            # Discard unused blocks, including removed build-time keys/package caches.
            run('fstrim', '--verbose', root)
        # Boot tests operate on an isolated copy and never leave test data in the release.
        from boot_test import verify
        report['boot_test'] = verify(image, profile, directory)
        if git('rev-parse', 'HEAD') != commit or git('status', '--porcelain', '--untracked-files=no'):
            raise ValueError('Source changed during image validation; rebuild from a clean commit')
        destination = output / f'kanlo-{version()}-{profile}.img.xz'
        with destination.open('wb') as stream:
            run('xz', '--compress', '--stdout', '-T2', '-6', image, stdout=stream)
        report['image'] = destination.name
        report['sha256'] = checksum(destination)
        report['raw_size'] = image.stat().st_size
        (output / f'kanlo-{version()}-{profile}.image.json').write_text(json.dumps(report, indent=2) + '\n')
        print('PASS: built and boot-verified', destination)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--profile', choices=PROFILES, required=True)
    parser.add_argument('--package', type=Path, required=True)
    parser.add_argument('--cache', type=Path, default=Path('/cache'))
    parser.add_argument('--work', type=Path, default=Path('/work'))
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    build(args.profile, args.package.resolve(), args.cache.resolve(), args.work.resolve(), args.output.resolve())
