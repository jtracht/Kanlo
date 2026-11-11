"""Create and verify source-only plugin releases from the tagged commit."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import zipfile

from release_package import ROOT, file_sha256, git


COMMON = {
    'server.mjs': 'plugins/kanlo/server.mjs',
    'package.json': 'plugins/kanlo/package.json',
    'package-lock.json': 'plugins/kanlo/package-lock.json',
    'LICENSE': 'LICENSE',
    'NOTICE': 'NOTICE',
}
PLUGINS = {
    'claude-code': {
        '.claude-plugin/plugin.json': 'plugins/kanlo/.claude-plugin/plugin.json',
        '.mcp.json': 'plugins/kanlo/.mcp.json',
        'README.md': 'release/plugins/CLAUDE-CODE.md',
    },
    'chatgpt': {'README.md': 'release/plugins/CHATGPT.md'},
}


def source(commit, path):
    return subprocess.check_output(['git', 'show', f'{commit}:{path}'], cwd=ROOT)


def archives(tag, commit):
    match = re.fullmatch(r'plugins-v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', tag)
    if not match:
        raise ValueError('Expected a plugin tag plugins-vX.Y.Z')
    version = tag.removeprefix('plugins-v')
    for manifest in ('package.json', 'package-lock.json', '.claude-plugin/plugin.json'):
        if json.loads(source(commit, f'plugins/kanlo/{manifest}'))['version'] != version:
            raise ValueError(f'Plugin tag and {manifest} version differ')
    for client, files in PLUGINS.items():
        name = f'kanlo-{client}-{version}-source'
        yield name, {f'{name}/{target}': path for target, path in sorted((COMMON | files).items())}


def bundle(output, tag):
    output.mkdir(parents=True, exist_ok=True)
    commit = git('rev-parse', 'HEAD')
    checksums = []
    for name, files in archives(tag, commit):
        path = output / f'{name}.zip'
        with zipfile.ZipFile(path, 'w') as archive:
            for target, original in files.items():
                entry = zipfile.ZipInfo(target)
                entry.create_system = 3
                entry.external_attr = 0o100644 << 16
                archive.writestr(entry, source(commit, original), compress_type=zipfile.ZIP_DEFLATED)
        checksums.append(f'{file_sha256(path)}  {path.name}\n')
    (output / 'SHA256SUMS').write_text(''.join(checksums), encoding='ascii', newline='\n')


def checked_plugin_files(output, tag, commit):
    files = []
    checksums = []
    for name, members in archives(tag, commit):
        path = output / f'{name}.zip'
        with zipfile.ZipFile(path) as archive:
            if archive.namelist() != list(members) or any(
                archive.read(target) != source(commit, original)
                for target, original in members.items()
            ):
                raise ValueError(f'{path.name} differs from tagged sources')
        files.append(path)
        checksums.append(f'{file_sha256(path)}  {path.name}\n')
    sums = output / 'SHA256SUMS'
    if sums.read_bytes() != ''.join(checksums).encode('ascii'):
        raise ValueError('Plugin release checksums differ')
    return [*files, sums]


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--tag', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    bundle(args.output, args.tag)
