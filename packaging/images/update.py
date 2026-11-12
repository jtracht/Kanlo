"""Explicit Kanlo server upgrades with a private, verified SQLite rollback snapshot."""
import argparse
import contextlib
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import sqlite3
import stat
import subprocess
import tarfile
import tempfile
import time
import urllib.request
import uuid

BINARY = Path('/usr/local/lib/kanlo/kanlo-server')
DATABASE = Path('/var/lib/kanlo/kanlo.sqlite3')
STATE = Path('/var/lib/kanlo-updates')
SERVICE = 'kanlo-server.service'
HEALTH = 'http://127.0.0.1/api/v1/health'


def checksum(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def sync_directory(path):
    descriptor = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def replace_file(source, destination, mode, owner=None):
    with tempfile.NamedTemporaryFile(dir=destination.parent, delete=False) as output:
        temporary = Path(output.name)
        try:
            with source.open('rb') as stream:
                shutil.copyfileobj(stream, output)
            output.flush()
            os.fchmod(output.fileno(), mode)
            if owner is not None:
                os.fchown(output.fileno(), *owner)
            os.fsync(output.fileno())
            os.replace(temporary, destination)
            sync_directory(destination.parent)
        finally:
            temporary.unlink(missing_ok=True)


def copy_database(source, destination):
    with contextlib.closing(sqlite3.connect(source.as_uri() + '?mode=ro', uri=True)) as original:
        with contextlib.closing(sqlite3.connect(destination)) as backup:
            original.backup(backup)
            backup.execute('PRAGMA journal_mode=DELETE')
            if backup.execute('PRAGMA integrity_check').fetchall() != [('ok',)]:
                raise ValueError('SQLite integrity verification failed')
    with destination.open('rb') as stream:
        os.fsync(stream.fileno())


def unpack(package, expected, architecture, work):
    if not re.fullmatch('[0-9a-f]{64}', expected):
        raise ValueError('Provide a SHA-256 checksum from the trusted release')
    # Copy before verification so the inspected file cannot change afterwards.
    local = work / 'package.tar.gz'
    with package.open('rb') as source, local.open('wb') as target:
        shutil.copyfileobj(source, target)
    if checksum(local) != expected:
        raise ValueError('Release package checksum mismatch')
    with tarfile.open(local, 'r:gz') as archive:
        members = archive.getmembers()
        if len(members) != len({member.name for member in members}):
            raise ValueError('Duplicate release package entry')
        manifests = [member for member in members if re.fullmatch(
            r'kanlo-[0-9A-Za-z.+-]+-linux-[a-z0-9]+/BUILD\.json', member.name)]
        if len(manifests) != 1 or not manifests[0].isfile() or manifests[0].size > 8192:
            raise ValueError('Invalid release manifest')
        manifest = manifests[0]
        build = json.load(archive.extractfile(manifest))
        version = build['version']
        if (build['schema_version'] != 1 or build['arch'] != architecture
                or build['minimum_os'] != 'Debian 13'
                or manifest.name != f'kanlo-{version}-linux-{architecture}/BUILD.json'):
            raise ValueError('Release package does not match this installation')
        member = archive.getmember(manifest.name.removesuffix('BUILD.json') + 'bin/kanlo-server')
        if not member.isfile() or not 0 < member.size <= 128 * 1024 * 1024:
            raise ValueError('Invalid release executable')
        candidate = work / 'kanlo-server'
        with archive.extractfile(member) as source, candidate.open('wb') as target:
            shutil.copyfileobj(source, target)
        if checksum(candidate) != build['binary_sha256']:
            raise ValueError('Release executable checksum mismatch')
    candidate.chmod(0o700)
    return candidate, version


class System:
    def validate_data_path(self, database):
        pid = subprocess.check_output(
            ['systemctl', 'show', '--value', '--property=MainPID', SERVICE], text=True).strip()
        if not pid.isdecimal() or pid == '0':
            raise ValueError('Start the installed Kanlo service before updating')
        environment = Path('/proc', pid, 'environ').read_bytes().split(b'\0')
        if os.fsencode('KANLO_DATA_DIR=' + str(database.parent)) not in environment:
            raise ValueError('The installed service uses a different data directory')

    def stop(self):
        subprocess.run(['systemctl', 'stop', SERVICE], check=True)

    def start(self):
        subprocess.run(['systemctl', 'reset-failed', SERVICE], stdout=subprocess.DEVNULL,
                       stderr=subprocess.DEVNULL, check=False)
        subprocess.run(['systemctl', 'start', SERVICE], check=True)

    def health(self, expected=None):
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            try:
                with urllib.request.urlopen(HEALTH, timeout=2) as response:
                    health = json.load(response)
                if health['status'] == 'ok' and (expected is None or health['version'] == expected):
                    return health['version']
            except (OSError, ValueError, KeyError):
                pass
            time.sleep(0.5)
        raise RuntimeError('Kanlo health check timed out')

    def preflight(self, candidate, directory):
        result = subprocess.run([candidate, '--check'], cwd=directory, check=True,
                                env={'KANLO_DATA_DIR': str(directory)}, capture_output=True,
                                text=True, timeout=120)
        if result.stdout.strip() != 'ok':
            raise ValueError('Candidate could not migrate and verify a copy of the database')


class Updater:
    def __init__(self, binary, database, state, system):
        self.binary = binary
        self.database = database
        self.state = state
        self.system = system

    @contextlib.contextmanager
    def locked(self):
        import fcntl

        self.state.mkdir(mode=0o700, exist_ok=True)
        info = self.state.lstat()
        if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.geteuid() or info.st_mode & 0o077:
            raise ValueError('Update state must be an owner-only directory')
        with (self.state / 'lock').open('ab') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            yield

    def snapshot(self, slot, version):
        for path in (self.binary, self.database):
            if not stat.S_ISREG(path.lstat().st_mode):
                raise ValueError('Kanlo executable and database must be regular files')
        database = self.database.stat()
        with tempfile.TemporaryDirectory(dir=self.state) as temporary:
            source = Path(temporary)
            staged = source / 'snapshot'
            staged.mkdir(mode=0o700)
            shutil.copyfile(self.binary, staged / 'kanlo-server')
            copy_database(self.database, staged / 'kanlo.sqlite3')
            metadata = {'version': version, 'uid': database.st_uid, 'gid': database.st_gid,
                        'binary_sha256': checksum(staged / 'kanlo-server'),
                        'database_sha256': checksum(staged / 'kanlo.sqlite3')}
            (staged / 'snapshot.json').write_text(json.dumps(metadata), encoding='utf-8')
            for path in staged.iterdir():
                path.chmod(0o600)
                with path.open('rb') as stream:
                    os.fsync(stream.fileno())
            sync_directory(staged)
            staged.rename(slot)
            sync_directory(self.state)

    def restore(self, slot):
        metadata = json.loads((slot / 'snapshot.json').read_text(encoding='utf-8'))
        binary, database = slot / 'kanlo-server', slot / 'kanlo.sqlite3'
        if checksum(binary) != metadata['binary_sha256'] or checksum(database) != metadata['database_sha256']:
            raise ValueError('Rollback snapshot failed checksum verification')
        self.system.stop()
        for suffix in ('-wal', '-shm', '-journal'):
            self.database.with_name(self.database.name + suffix).unlink(missing_ok=True)
        with tempfile.TemporaryDirectory(dir=self.state) as temporary:
            restored = Path(temporary) / 'kanlo.sqlite3'
            shutil.copyfile(database, restored)
            # Clients must review pending edits from the state that was rolled back.
            with contextlib.closing(sqlite3.connect(restored)) as connection, connection:
                changed = connection.execute(
                    "UPDATE metadata SET value=? WHERE key='epoch'", (str(uuid.uuid4()),))
                if changed.rowcount != 1:
                    raise ValueError('Rollback snapshot lacks its database epoch')
            replace_file(restored, self.database, 0o600, (metadata['uid'], metadata['gid']))
        replace_file(binary, self.binary, 0o755)
        self.system.start()
        self.system.health(metadata['version'])

    def apply(self, package, expected, architecture):
        with self.locked():
            previous = self.state / 'rollback'
            pending = self.state / 'pending'
            if pending.exists():
                raise ValueError('An interrupted update must be rolled back first')
            with tempfile.TemporaryDirectory(dir=self.state) as temporary:
                work = Path(temporary)
                candidate, version = unpack(package, expected, architecture, work)
                old_version = self.system.health()
                self.system.validate_data_path(self.database)
                self.system.stop()
                try:
                    self.snapshot(pending, old_version)
                    staged = work / 'preflight'
                    staged.mkdir(mode=0o700)
                    shutil.copyfile(pending / 'kanlo.sqlite3', staged / 'kanlo.sqlite3')
                    self.system.preflight(candidate, staged)
                    replace_file(candidate, self.binary, 0o755)
                    self.system.start()
                    self.system.health(version)
                except BaseException:
                    if pending.exists():
                        self.restore(pending)
                        shutil.rmtree(pending)
                        sync_directory(self.state)
                    else:
                        self.system.start()
                    raise
                if previous.exists():
                    shutil.rmtree(previous)
                pending.rename(previous)
                sync_directory(self.state)

    def rollback(self):
        with self.locked():
            pending, previous = self.state / 'pending', self.state / 'rollback'
            slot = pending if pending.is_dir() else previous
            if not slot.is_dir():
                raise ValueError('No rollback snapshot available')
            self.restore(slot)
            shutil.rmtree(slot)
            sync_directory(self.state)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    apply = commands.add_parser('apply')
    apply.add_argument('package', type=Path)
    apply.add_argument('--sha256', required=True)
    commands.add_parser('rollback')
    args = parser.parse_args()
    if os.geteuid() != 0:
        parser.error('Run this command as the device administrator')
    updater = Updater(BINARY, DATABASE, STATE, System())
    try:
        if args.command == 'apply':
            architecture = subprocess.check_output(['dpkg', '--print-architecture'], text=True).strip()
            updater.apply(args.package, args.sha256, architecture)
        else:
            updater.rollback()
    except (OSError, ValueError, KeyError, RuntimeError, sqlite3.Error, tarfile.TarError,
            subprocess.SubprocessError) as error:
        parser.exit(1, f'Kanlo update: {error}\n')
    print('Kanlo server is healthy.')


if __name__ == '__main__':
    main()
