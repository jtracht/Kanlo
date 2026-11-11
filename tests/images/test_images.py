from contextlib import nullcontext
import json
from pathlib import Path
import sys
import tempfile
import threading
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'packaging/images'))
import appliance
import display
import build


class Appliance(unittest.TestCase):
    def test_display_stream_updates_affected_boards_and_reconciles_on_reconnect(self):
        frames = [b': keepalive\n', b'\n', b'event: change\n',
                  b'data: {"boards":["a"]}\n', b'\n']
        bootstrap = {'boards': [{'id': 'a'}, {'id': 'b'}], 'instance_id': 'server',
                     'epoch': 'first', 'event_cursor': 10}
        stop = threading.Event()
        generations = iter(['A1', 'B1', 'A2', 'A3', 'B3'])
        requests = []
        publications = []

        def fetch(path):
            requests.append(path)
            if path == 'bootstrap':
                return bootstrap
            return {'workspace': next(generations)}

        class Mailbox:
            def get_nowait(self):
                return None

            def put_nowait(self, state):
                publications.append(state)

        def connect(url, timeout):
            self.assertTrue(url.endswith('/events?after=10'))
            if requests.count('bootstrap') == 3:
                stop.set()
                return nullcontext([])
            return nullcontext(frames)

        with (
            patch.object(display, 'fetch', side_effect=fetch),
            patch.object(display.urllib.request, 'urlopen', side_effect=connect),
            patch.object(stop, 'wait'),
        ):
            display.reader(Mailbox(), stop)
        self.assertEqual(requests, ['bootstrap', 'boards/a/snapshot', 'boards/b/snapshot',
                                    'bootstrap', 'boards/a/snapshot',
                                    'bootstrap', 'boards/a/snapshot', 'boards/b/snapshot'])
        self.assertEqual([state for state in publications if state[1]],
                         [(['A1', 'B1'], True), (['A2', 'B1'], True), (['A3', 'B3'], True)])

    def test_configuration_rejects_unknown_fields_and_unsafe_values(self):
        with patch.object(Path, 'is_file', return_value=True):
            valid = appliance.validate({'hostname': 'kanlo-pi', 'timezone': 'Europe/Berlin', 'wifi': {'ssid': 'Office; WiFi', 'password': 'a safe passphrase', 'country': 'DE'}})
            self.assertEqual(valid['wifi']['ssid'], 'Office; WiFi')
            for config in [{'hostname': 'x\nKANLO_BIND=evil'}, {'hostname': 'a; reboot'}, {'timezone': '../../etc/shadow'}, {'display': 'yes'}, {'password': 'factory'}, {'ssh_authorized_key': 'command="reboot" ssh-ed25519 AAAA'}, {'wifi': {'ssid': 'a', 'password': 'short', 'country': 'DE'}}, {'wifi': {'ssid': 'a\n', 'password': '12345678', 'country': 'DE'}}]:
                with self.subTest(config=config), self.assertRaises(ValueError):
                    appliance.validate(config)

    def test_network_authorities_are_exact_local_hosts(self):
        hosts = appliance.authorities(['192.168.1.25', '10.0.2.15', '127.0.0.1', '8.8.8.8', 'fe80::1'], 'kanlo-ab12cd')
        self.assertEqual(hosts, ['10.0.2.15:8080', '192.168.1.25:8080', 'kanlo-ab12cd.local:8080', 'kanlo-ab12cd:8080'])

    def test_atomic_private_configuration_does_not_leave_temporary_secrets(self):
        with tempfile.TemporaryDirectory() as directory:
            file = Path(directory) / 'private/config'
            appliance.atomic(file, 'first')
            appliance.atomic(file, 'second')
            self.assertEqual(file.read_text(), 'second')
            self.assertEqual([p.name for p in file.parent.iterdir()], ['config'])

    def test_native_board_display_excludes_trash_and_archive_and_sorts(self):
        column = {'id': 'a', 'position': 1}
        tickets = [{'column_id': 'a', 'details': {'trashed': False, 'archived': False, 'rank': 10}}, {'column_id': 'a', 'details': {'trashed': True, 'archived': False, 'rank': 1}}, {'column_id': 'a', 'details': {'trashed': False, 'archived': True, 'rank': 2}}, {'column_id': 'a', 'details': {'trashed': False, 'archived': False, 'rank': 3}}]
        shown = display.visible({'columns': [column], 'tickets': tickets})
        self.assertEqual([t['details']['rank'] for t in shown[0][1]], [3, 10])

    def test_display_detects_hdmi_and_composite_without_edid(self):
        for connectors, expected in [
            ([('card0-HDMI-A-1', 'connected', ['1920x1080'])], True),
            ([('card0-HDMI-A-1', 'disconnected', []), ('card1-Composite-1', 'unknown', ['720x576'])], True),
            ([('card0-HDMI-A-1', 'unknown', ['1920x1080']), ('card1-Composite-1', 'unknown', [])], False),
        ]:
            with self.subTest(connectors=connectors), patch.object(appliance, 'display_connectors', return_value=connectors):
                self.assertEqual(appliance.graphical_output(), expected)

    def test_display_launcher_allows_virtual_terminal_switching_and_falls_back(self):
        with (
            patch.object(appliance, 'graphical_output', return_value=True),
            patch.object(display, 'wait_for_seat'),
            patch.object(display.subprocess, 'run') as cage,
            patch.object(display, 'console_board') as fallback,
        ):
            cage.return_value.returncode = 1
            display.launch()
            self.assertEqual(cage.call_args.args[0][:3], ['/usr/bin/cage', '-s', '--'])
            fallback.assert_called_once_with()

    def test_wifi_menu_starts_local_network_selector_without_credentials_in_command(self):
        with (
            patch('builtins.input', return_value='de'),
            patch.object(appliance, 'set_wifi_country') as country,
            patch.object(appliance, 'run') as run,
            patch.object(appliance, 'print_web_addresses') as addresses,
        ):
            appliance.setup_wifi()
            country.assert_called_once_with('DE')
            self.assertEqual(run.call_args_list[1].args, ('/usr/bin/nmtui-connect',))
            self.assertEqual(run.call_args_list[2].args, ('systemctl', 'restart', 'kanlo-network.service'))
            addresses.assert_called_once_with()

    def test_all_four_official_bases_are_immutable_and_checked(self):
        self.assertEqual(set(build.PROFILES), {'rpi-armhf', 'rpi-arm64', 'generic-amd64', 'generic-arm64'})
        for profile in build.PROFILES.values():
            self.assertNotIn('/latest/', profile['url'])
            self.assertRegex(profile['checksum'], r'^[0-9a-f]+$')
            self.assertEqual(len(profile['checksum']), 64 if profile['hash_algorithm'] == 'sha256' else 128)

    def test_native_pagination_includes_backlog_and_every_ticket(self):
        tickets = [{'id': str(i), 'column_id': None, 'details': {'trashed': False, 'archived': False, 'rank': i}} for i in range(31)]
        pages = display.paginate({'columns': [], 'tickets': tickets}, 3, 4)
        shown = [t['id'] for page in pages for _, group, _ in page for t in group]
        self.assertEqual(shown, [str(i) for i in range(31)])
        self.assertEqual(len(pages), 8)

    def test_builder_refuses_block_devices_and_files_outside_work(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory) / 'work'; work.mkdir()
            outside = Path(directory) / 'outside.img'; outside.write_bytes(b'fixture')
            with self.assertRaises(ValueError), build.mounted(outside, work):
                pass

    def test_shipped_overlay_does_not_include_the_boot_test_hook(self):
        files = list((ROOT / 'packaging/images/overlay').rglob('*'))
        self.assertFalse(any('boot_guest' in str(p) or 'image-test' in str(p) for p in files))
        self.assertNotIn('chromium', (ROOT / 'packaging/images/customize.sh').read_text())


if __name__ == '__main__':
    unittest.main()
