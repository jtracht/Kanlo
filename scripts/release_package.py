"""Deterministic, allowlisted release archives; no third-party Python packages."""
import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import tarfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {
    'x86_64-unknown-linux-gnu': ('amd64', 62, 2),
    'aarch64-unknown-linux-gnu': ('arm64', 183, 2),
    'armv7-unknown-linux-gnueabihf': ('armhf', 40, 1),
}
TAG = re.compile(r'^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-(dev|beta|rc)\.(0|[1-9]\d*))?$')


def version():
    return tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']['package']['version']


def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT).decode().strip()


def file_sha256(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(4 * 1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def image_files(output, commit):
    profiles = json.loads((ROOT / 'packaging/images/bases.json').read_text())['profiles']
    files, images = [], []
    for profile, base in sorted(profiles.items()):
        prefix = f'kanlo-{version()}-{profile}'
        image = output / (prefix + '.img.xz')
        evidence = output / (prefix + '.image.json')
        if image.is_symlink() or evidence.is_symlink():
            raise ValueError('Image inputs must be regular files')
        report = json.loads(evidence.read_text())
        boot = report.get('boot_test', {})
        if report.get('source_commit') != commit or report.get('version') != version() or report.get('profile') != profile or report.get('base') != base or report.get('image') != image.name:
            raise ValueError('Image provenance/version/base mismatch')
        if boot.get('status') != 'passed' or [b.get('phase') for b in boot.get('boots', [])] != ['firstboot', 'reboot'] or any(b.get('sqlite_integrity') != 'ok' or not b.get('root_expanded') or not b.get('lan_http_verified') or not b.get('ram_swap_verified') for b in boot['boots']):
            raise ValueError('Two successful image boots are required')
        if not re.fullmatch(r'[0-9a-f]{64}', boot.get('emulator_binary_sha256', '')) or base['arch'] != 'amd64' and boot.get('emulator_source_commit') != '7e0b2296c5d8e6ae0325f32c2c879ca2aba83ed5':
            raise ValueError('Pinned emulator provenance is required')
        with image.open('rb') as stream:
            if stream.read(6) != b'\xfd7zXZ\x00':
                raise ValueError('Expected an XZ-compressed raw disk image')
        if image.stat().st_size >= 2**31 or file_sha256(image) != report.get('sha256'):
            raise ValueError('Image checksum/asset size mismatch')
        files.extend([image, evidence])
        images.append({k: v for k, v in report.items() if k != 'packages'} | {'evidence': evidence.name})
    return files, images


def validate_tag(tag):
    match = TAG.fullmatch(tag)
    if not match or tag != f'v{version()}':
        raise ValueError('Tag must be v<Cargo version>, optionally -dev.N, -beta.N or -rc.N')
    prerelease = match[4] is not None
    if not prerelease:
        gates = json.loads((ROOT / 'release/readiness.json').read_text())['stable_gates']
        if not gates:
            raise ValueError('Stable readiness gates are missing')
        for name, evidence in gates.items():
            if not isinstance(evidence, str):
                raise ValueError(f'Stable gate not accepted: {name}')
            path = (ROOT / evidence).resolve()
            if not path.is_relative_to(ROOT) or not path.is_file():
                raise ValueError(f'Stable evidence missing: {name}')
            report = json.loads(path.read_text())
            if report.get('status') != 'passed' or report.get('source_commit') != git('rev-parse', 'HEAD') or not report.get('approved_by'):
                raise ValueError(f'Stable evidence does not approve this commit: {name}')
        if 'LICENSE REVIEW PENDING' in (ROOT / 'LICENSE').read_text():
            raise ValueError('Public license approval is still pending')
    if (ROOT / 'release/NOTES.md').read_text().splitlines()[0] != f'# Kanlo {version()}':
        raise ValueError('Release notes must match the Cargo version')
    return prerelease


def elf(data, target):
    _, machine, bits = TARGETS[target]
    if len(data) < 20 or data[:4] != b'\x7fELF' or data[4] != bits or data[5] != 1 or int.from_bytes(data[18:20], 'little') != machine:
        raise ValueError(f'Binary architecture does not match {target}')


def archive(destination, files, epoch):
    destination.parent.mkdir(parents=True, exist_ok=True)
    with destination.open('wb') as raw, gzip.GzipFile(fileobj=raw, mode='wb', filename='', mtime=epoch) as compressed, tarfile.open(fileobj=compressed, mode='w') as tar:
        for name, data, mode in sorted(files):
            item = tarfile.TarInfo(name)
            item.size, item.mode, item.mtime = len(data), mode, epoch
            item.uid = item.gid = 0
            item.uname = item.gname = ''
            tar.addfile(item, io.BytesIO(data))


def package(binary, target, output):
    if binary.is_symlink():
        raise ValueError('Binary must be a regular file')
    data = binary.read_bytes()
    elf(data, target)
    commit = os.environ.get('SOURCE_COMMIT') or git('rev-parse', 'HEAD')
    if not re.fullmatch(r'[0-9a-f]{40}', commit):
        raise ValueError('A full source commit is required')
    epoch = int(os.environ.get('SOURCE_DATE_EPOCH') or git('log', '-1', '--format=%ct'))
    arch = TARGETS[target][0]
    prefix = f'kanlo-{version()}-linux-{arch}'
    build = {'schema_version': 1, 'version': version(), 'target': target, 'arch': arch, 'source_commit': commit, 'source_date_epoch': epoch, 'binary_sha256': hashlib.sha256(data).hexdigest(), 'minimum_os': 'Debian 13', 'native_tv': False, 'physical_device_certified': False}
    files = [(f'{prefix}/bin/kanlo-server', data, 0o755), (f'{prefix}/BUILD.json', (json.dumps(build, indent=2) + '\n').encode(), 0o644)]
    paths = {
        'LICENSE': 'LICENSE', 'NOTICE': 'NOTICE', 'CHANGELOG.md': 'CHANGELOG.md',
        'release/NOTES.md': 'RELEASE-NOTES.md', 'release/INSTALL-DE.md': 'INSTALL-DE.md', 'release/INSTALL-EN.md': 'INSTALL-EN.md',
        'release/IMAGES-DE.md': 'IMAGES-DE.md', 'release/IMAGES-EN.md': 'IMAGES-EN.md',
        'docs/de/BETRIEB.md': 'docs/de/BETRIEB.md', 'docs/en/README.md': 'docs/en/README.md',
        'packaging/systemd/kanlo-server.service': 'systemd/kanlo-server.service', 'packaging/sysusers.d/kanlo.conf': 'sysusers.d/kanlo.conf',
    }
    paths.update({str(path.relative_to(ROOT)).replace('\\', '/'): str(path.relative_to(ROOT)).replace('\\', '/') for path in (ROOT / 'docs/licenses').rglob('*') if path.is_file()})
    for source, name in paths.items():
        path = ROOT / source
        if path.is_symlink():
            raise ValueError('Package input must not be a symlink')
        files.append((f'{prefix}/{name}', path.read_bytes(), 0o644))
    result = output / f'{prefix}.tar.gz'
    archive(result, files, epoch)
    return result


def inspect(path):
    with tarfile.open(path) as tar:
        members = tar.getmembers()
        roots = {item.name.split('/')[0] for item in members}
        if len(roots) != 1 or len({item.name for item in members}) != len(members) or any(not item.isfile() or item.name.startswith('/') or '..' in Path(item.name).parts for item in members):
            raise ValueError('Unsafe or malformed release archive')
        prefix = roots.pop()
        build = json.load(tar.extractfile(f'{prefix}/BUILD.json'))
        if build['version'] != version() or build['target'] not in TARGETS or prefix != f"kanlo-{version()}-linux-{TARGETS[build['target']][0]}":
            raise ValueError('Package version/target mismatch')
        data = tar.extractfile(f'{prefix}/bin/kanlo-server').read()
        elf(data, build['target'])
        if hashlib.sha256(data).hexdigest() != build['binary_sha256']:
            raise ValueError('Embedded binary checksum mismatch')
        for required in ['LICENSE', 'NOTICE', 'INSTALL-DE.md', 'INSTALL-EN.md', 'RELEASE-NOTES.md', 'docs/licenses/inventory.json']:
            tar.getmember(f'{prefix}/{required}')
        return build


def bundle(output, tag):
    validate_tag(tag)
    packages = sorted(output.glob(f'kanlo-{version()}-linux-*.tar.gz'))
    builds = [inspect(path) for path in packages]
    commit = git('rev-parse', 'HEAD')
    if len(builds) != 3 or {b['target'] for b in builds} != set(TARGETS) or any(b['source_commit'] != commit for b in builds):
        raise ValueError('Exactly three packages from this commit are required')
    image_assets, images = image_files(output, commit)
    epoch = int(git('log', '-1', '--format=%ct'))
    source = output / f'kanlo-{version()}-source.tar.gz'
    raw = subprocess.check_output(['git', 'archive', '--format=tar', f'--prefix=kanlo-{version()}-source/', 'HEAD'], cwd=ROOT)
    with source.open('wb') as stream, gzip.GzipFile(fileobj=stream, mode='wb', filename='', mtime=epoch) as compressed:
        compressed.write(raw)
    manifest = output / 'release-manifest.json'
    manifest.write_text(json.dumps({'schema_version': 2, 'tag': tag, 'source_commit': commit, 'prerelease': '-' in tag, 'platforms': builds, 'images': images}, indent=2) + '\n')
    files = [*packages, *image_assets, source, manifest]
    (output / 'SHA256SUMS').write_text(''.join(f'{file_sha256(path)}  {path.name}\n' for path in sorted(files)))
    return files


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest='command', required=True)
    p = sub.add_parser('validate'); p.add_argument('--tag', required=True)
    p = sub.add_parser('package'); p.add_argument('--binary', type=Path, required=True); p.add_argument('--target', choices=TARGETS, required=True); p.add_argument('--output', type=Path, required=True)
    p = sub.add_parser('bundle'); p.add_argument('--output', type=Path, required=True); p.add_argument('--tag', required=True)
    args = parser.parse_args()
    if args.command == 'validate':
        print('prerelease' if validate_tag(args.tag) else 'stable')
    elif args.command == 'package':
        print(package(args.binary, args.target, args.output))
    else:
        bundle(args.output, args.tag)
