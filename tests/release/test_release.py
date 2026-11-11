import hashlib
import io
import json
import lzma
import os
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import urllib.error

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
import release_package as package
import publish_release as publisher


def binary(target):
    _, machine, bits = package.TARGETS[target]
    data = bytearray(64)
    data[:6] = b'\x7fELF' + bytes([bits, 1])
    data[18:20] = machine.to_bytes(2, 'little')
    return bytes(data)


class Packages(unittest.TestCase):
    def test_tag_must_match_manifest_and_channel(self):
        self.assertTrue(package.validate_tag('v' + package.version()))
        for tag in ['release', 'v999.0.0-dev.1', 'v0.1.0-dev.01', 'v0.1.0-nightly.1', 'v0.1.0-dev.1; echo nope']:
            with self.assertRaises(ValueError):
                package.validate_tag(tag)

    def test_stable_fails_closed_without_hardware_and_legal_evidence(self):
        with patch.object(package, 'version', return_value='1.0.0'):
            with self.assertRaisesRegex(ValueError, 'Stable gate'):
                package.validate_tag('v1.0.0')

    def test_all_architectures_are_checked(self):
        for target in package.TARGETS:
            package.elf(binary(target), target)
            with self.assertRaises(ValueError):
                package.elf(b'not an ELF binary', target)
        with self.assertRaises(ValueError):
            package.elf(binary('x86_64-unknown-linux-gnu'), 'aarch64-unknown-linux-gnu')

    def test_archives_are_deterministic_allowlisted_and_inspected(self):
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, {'SOURCE_COMMIT': 'a' * 40, 'SOURCE_DATE_EPOCH': '1700000000'}):
            directory = Path(directory)
            executable = directory / 'binary'; executable.write_bytes(binary('x86_64-unknown-linux-gnu'))
            first = package.package(executable, 'x86_64-unknown-linux-gnu', directory / 'a')
            second = package.package(executable, 'x86_64-unknown-linux-gnu', directory / 'b')
            self.assertEqual(first.read_bytes(), second.read_bytes())
            build = package.inspect(first)
            self.assertEqual(build['source_commit'], 'a' * 40)
            with tarfile.open(first) as tar:
                self.assertFalse(any('/data/' in item.name or item.name.endswith('.env') or item.issym() for item in tar))

    def test_incomplete_platform_set_cannot_be_released(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, 'Exactly three'):
                package.bundle(Path(directory), 'v' + package.version())

    def test_real_bundle_checks_all_platforms_and_rejects_tampered_download(self):
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, {'SOURCE_COMMIT': package.git('rev-parse', 'HEAD'), 'SOURCE_DATE_EPOCH': '1700000000'}):
            directory = Path(directory)
            for target in package.TARGETS:
                executable = directory / target
                executable.write_bytes(binary(target))
                package.package(executable, target, directory)
            tag = 'v' + package.version()
            profiles = json.loads((package.ROOT / 'packaging/images/bases.json').read_text())['profiles']
            for profile, base in profiles.items():
                image = directory / f'kanlo-{package.version()}-{profile}.img.xz'
                image.write_bytes(lzma.compress(b'fixture image, not boot evidence'))
                evidence = image.with_name(f'kanlo-{package.version()}-{profile}.image.json')
                evidence.write_text(json.dumps({'profile': profile, 'version': package.version(), 'source_commit': package.git('rev-parse', 'HEAD'), 'base': base, 'image': image.name, 'sha256': package.file_sha256(image), 'boot_test': {'status': 'passed', 'boots': [{'phase': phase, 'sqlite_integrity': 'ok', 'root_expanded': True} for phase in ['firstboot', 'reboot']]}}))
                fixture = json.loads(evidence.read_text())
                fixture['boot_test'].update(emulator_binary_sha256='0' * 64, emulator_source_commit=None if base['arch'] == 'amd64' else '7e0b2296c5d8e6ae0325f32c2c879ca2aba83ed5')
                for boot in fixture['boot_test']['boots']:
                    boot.update(lan_http_verified=True, ram_swap_verified=True, independent_service_restarts=True, live_display_verified=True)
                evidence.write_text(json.dumps(fixture))
            original = evidence.read_text()
            rejected = json.loads(original)
            rejected['boot_test']['status'] = 'failed'
            evidence.write_text(json.dumps(rejected))
            with self.assertRaisesRegex(ValueError, 'Two successful image boots'):
                package.bundle(directory, tag)
            evidence.write_text(original)
            package.bundle(directory, tag)
            self.assertEqual(len(publisher.checked_files(directory, tag)), 14)
            archive = next(directory.glob('*-linux-amd64.tar.gz'))
            archive.write_bytes(archive.read_bytes() + b'tampered')
            with self.assertRaisesRegex(ValueError, 'checksum mismatch'):
                publisher.checked_files(directory, tag)

    def test_archive_traversal_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / 'bad.tar.gz'
            with tarfile.open(archive, 'w:gz') as tar:
                item = tarfile.TarInfo('../outside'); item.size = 1
                tar.addfile(item, io.BytesIO(b'x'))
            with self.assertRaises(ValueError):
                package.inspect(archive)

    def test_upload_streams_have_the_exact_declared_http_length(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'image.img.xz'
            payload = b'fixture payload\x00' * 1000
            path.write_bytes(payload)
            for server, base, release in [('https://github.com', 'https://api.github.com', {'upload_url': 'https://uploads.github.com/repos/owner/repo/releases/1/assets{?name,label}'}), ('https://gitea.example', 'https://gitea.example/api/v1', {'id': 1})]:
                api = publisher.API(server, base, 'owner/repo', 'fixture')
                def request(method, url, body, content_type, **kwargs):
                    self.assertEqual(method, 'POST')
                    self.assertFalse(isinstance(body, bytes))
                    wire = b''.join(body)
                    self.assertEqual(len(wire), kwargs['length'])
                    self.assertIn(payload, wire)
                    if 'multipart' in content_type:
                        self.assertIn(b'name="attachment"; filename="image.img.xz"', wire)
                    return {'id': 1}
                with patch.object(api, 'request', side_effect=request), patch.object(Path, 'read_bytes', side_effect=AssertionError('Do not buffer image uploads')):
                    api.upload(release, path)

    def test_authenticated_api_does_not_allow_foreign_hosts(self):
        api = publisher.API('https://gitea.example', 'https://gitea.example/api/v1', 'owner/repo', 'fixture')
        with self.assertRaises(ValueError):
            api.request('POST', 'https://attacker.invalid/upload', b'x')
        with self.assertRaises(ValueError):
            publisher.API('https://gitea.example', 'https://attacker.invalid/api', 'owner/repo', 'fixture')


class FakeAPI:
    def __init__(self):
        self.release = None
        self.assets = []
        self.contents = {}
        self.fail = False

    def request(self, method, path, body=None):
        if path == '':
            return {'private': True}
        if path.startswith('/releases/tags/'):
            if self.release is None:
                raise urllib.error.HTTPError(path, 404, '', {}, None)
            return self.release
        if path == '/releases' and method == 'POST':
            self.release = {**body, 'id': 1, 'html_url': 'https://gitea.example/release'}
            return self.release
        if path.endswith('/assets'):
            return self.assets
        if method == 'PATCH':
            self.release.update(body)
            return self.release
        raise AssertionError((method, path))

    def upload(self, release, path):
        if self.fail:
            raise OSError('fixture: network unavailable')
        asset = {'id': len(self.assets) + 1, 'name': path.name}
        self.assets.append(asset); self.contents[asset['id']] = path.read_bytes()
        return asset

    def download(self, release, asset, digest=False):
        data = self.contents[asset['id']]
        return hashlib.sha256(data).digest() if digest else data


class Publication(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.files = [Path(self.directory.name) / name for name in ['archive.tar.gz', 'SHA256SUMS']]
        for file in self.files:
            file.write_bytes(file.name.encode())
        self.patches = [patch.object(publisher, 'checked_files', return_value=self.files), patch.object(publisher, 'git', return_value='a' * 40)]
        for item in self.patches:
            item.start()
        self.api = FakeAPI()

    def tearDown(self):
        for item in self.patches:
            item.stop()
        self.directory.cleanup()

    def test_complete_upload_publishes_and_retry_is_idempotent(self):
        publisher.publish(self.api, 'v' + package.version(), Path(self.directory.name))
        self.assertFalse(self.api.release['draft'])
        self.assertTrue(self.api.release['prerelease'])
        publisher.publish(self.api, 'v' + package.version(), Path(self.directory.name))
        self.assertEqual(len(self.api.assets), 2)

    def test_network_failure_leaves_draft_retry_can_complete_it(self):
        self.api.fail = True
        with self.assertRaises(OSError):
            publisher.publish(self.api, 'v' + package.version(), Path(self.directory.name))
        self.assertTrue(self.api.release['draft'])
        self.api.fail = False
        publisher.publish(self.api, 'v' + package.version(), Path(self.directory.name))
        self.assertFalse(self.api.release['draft'])

    def test_different_existing_asset_is_never_overwritten(self):
        publisher.publish(self.api, 'v' + package.version(), Path(self.directory.name))
        self.api.contents[1] = b'changed'
        with self.assertRaisesRegex(ValueError, 'differs'):
            publisher.publish(self.api, 'v' + package.version(), Path(self.directory.name))
        self.assertEqual(len(self.api.assets), 2)

    def test_existing_release_for_another_commit_is_rejected(self):
        publisher.publish(self.api, 'v' + package.version(), Path(self.directory.name))
        self.api.release['target_commitish'] = 'b' * 40
        with self.assertRaisesRegex(ValueError, 'another commit'):
            publisher.publish(self.api, 'v' + package.version(), Path(self.directory.name))


if __name__ == '__main__':
    unittest.main()
