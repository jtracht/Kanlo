import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import sqlite3
import tarfile
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location(
    'kanlo_update', Path(__file__).resolve().parents[2] / 'packaging/images/update.py')
update = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(update)


class LocalService:
    def __init__(self, binary, database):
        self.binary, self.database = binary, database
        self.running = True
        self.fail_new_health = False

    def validate_data_path(self, database):
        assert database == self.database

    def stop(self):
        self.running = False

    def start(self):
        self.running = True
        if self.binary.read_bytes() == b'new':
            with sqlite3.connect(self.database) as connection:
                connection.execute('CREATE TABLE IF NOT EXISTS new_schema (entry TEXT)')
                connection.execute("INSERT INTO new_schema VALUES ('migrated')")

    def health(self, expected=None):
        version = {b'old': '1.0.0', b'new': '1.1.0'}[self.binary.read_bytes()]
        if not self.running or (expected and version != expected):
            raise RuntimeError('Unexpected service state')
        if self.fail_new_health and version == '1.1.0':
            raise RuntimeError('Injected health failure after migration')
        return version

    def preflight(self, candidate, directory):
        assert candidate.read_bytes() == b'new'
        with sqlite3.connect(directory / 'kanlo.sqlite3') as connection:
            connection.execute('CREATE TABLE preflight_only (entry TEXT)')


@unittest.skipUnless(os.name == 'posix', 'Linux update tooling')
class UpdateRecovery(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.binary = self.root / 'installed'
        self.binary.write_bytes(b'old')
        self.database = self.root / 'kanlo.sqlite3'
        with sqlite3.connect(self.database) as connection:
            connection.executescript("""
                CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT);
                INSERT INTO metadata VALUES ('epoch', 'old-epoch');
                CREATE TABLE tickets (title TEXT);
                INSERT INTO tickets VALUES ('Keep me');
            """)
        self.system = LocalService(self.binary, self.database)
        self.updater = update.Updater(self.binary, self.database, self.root / 'state', self.system)
        self.package = self.root / 'release.tar.gz'
        manifest = {'version': '1.1.0', 'arch': 'amd64', 'minimum_os': 'Debian 13',
                    'schema_version': 1, 'binary_sha256': hashlib.sha256(b'new').hexdigest()}
        with tarfile.open(self.package, 'w:gz') as archive:
            for name, data in [('BUILD.json', json.dumps(manifest).encode()), ('bin/kanlo-server', b'new')]:
                member = tarfile.TarInfo('kanlo-1.1.0-linux-amd64/' + name)
                member.size = len(data)
                archive.addfile(member, io.BytesIO(data))

    def apply(self):
        self.updater.apply(self.package, update.checksum(self.package), 'amd64')

    def assert_restored(self):
        self.assertEqual(self.system.health(), '1.0.0')
        with sqlite3.connect(self.database) as connection:
            self.assertEqual(connection.execute('PRAGMA integrity_check').fetchall(), [('ok',)])
            self.assertEqual(connection.execute('SELECT * FROM tickets').fetchall(), [('Keep me',)])
            self.assertNotEqual(connection.execute('SELECT value FROM metadata').fetchone(), ('old-epoch',))
            self.assertIsNone(connection.execute(
                "SELECT name FROM sqlite_master WHERE name IN ('new_schema', 'preflight_only')").fetchone())

    def test_explicit_rollback_reverts_schema_and_retains_data(self):
        self.apply()
        self.assertEqual(self.system.health(), '1.1.0')
        with sqlite3.connect(self.database) as connection:
            self.assertEqual(connection.execute('SELECT * FROM new_schema').fetchall(), [('migrated',)])
        self.updater.rollback()
        self.assert_restored()

    def test_failed_health_after_migration_rolls_back_automatically(self):
        self.system.fail_new_health = True
        with self.assertRaisesRegex(RuntimeError, 'Injected health failure'):
            self.apply()
        self.assert_restored()

    def test_checksum_rejection_leaves_service_running(self):
        with self.assertRaisesRegex(ValueError, 'checksum mismatch'):
            self.updater.apply(self.package, '0' * 64, 'amd64')
        self.assertEqual(self.system.health(), '1.0.0')
        self.assertFalse((self.root / 'state' / 'pending').exists())

    def test_interrupted_update_can_be_rolled_back_without_a_running_service(self):
        pending = self.root / 'state' / 'pending'
        with self.updater.locked():
            self.system.stop()
            self.updater.snapshot(pending, '1.0.0')
            self.binary.write_bytes(b'new')
        self.updater.rollback()
        self.assert_restored()
        self.assertFalse(pending.exists())


if __name__ == '__main__':
    unittest.main()
