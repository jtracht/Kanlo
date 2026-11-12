from contextlib import nullcontext
from datetime import datetime, timezone
from email.message import Message
import io
import json
from pathlib import Path
import subprocess
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
import tv_markdown


class Appliance(unittest.TestCase):
    def test_raspberry_pi_start_messages_use_separate_console(self):
        line = build.raspberry_pi_cmdline('console=serial0,115200 console=tty1 console=tty2 '
                                          'root=PARTUUID=abc systemd.show_status=1 loglevel=7')
        self.assertEqual(line, 'console=serial0,115200 root=PARTUUID=abc '
                               'console=tty3 loglevel=3 systemd.show_status=no\n')

    def test_first_wifi_setup_runs_once_and_can_be_skipped(self):
        with tempfile.TemporaryDirectory() as directory:
            state = Path(directory)
            (state / 'provisioned.json').write_text('{}')
            with patch.object(appliance, 'STATE', state), patch.object(appliance, 'has_display', return_value=True), patch.object(appliance, 'run') as command, patch('builtins.input', return_value='0'), patch('builtins.print'):
                self.assertTrue(appliance.needs_welcome())
                appliance.welcome()
                self.assertEqual([call.args for call in command.call_args_list], [
                    ('chvt', '1'), ('systemctl', '--no-block', 'start', 'kanlo-tv.service'),
                ])
                self.assertFalse(appliance.needs_welcome())
            (state / 'welcome-complete').unlink()
            (state / 'provisioned.json').write_text('{"wifi_configured": true}')
            with patch.object(appliance, 'STATE', state), patch.object(appliance, 'has_display', return_value=True):
                self.assertFalse(appliance.needs_welcome())
            with patch.object(appliance, 'STATE', state), patch.object(appliance, 'has_display', return_value=False):
                self.assertFalse(appliance.needs_welcome())

    def test_successful_first_wifi_connection_continues_to_board(self):
        with tempfile.TemporaryDirectory() as directory:
            state = Path(directory)
            connected = subprocess.CompletedProcess([], 0, 'ethernet:unavailable\nwifi:connected\n', '')
            with patch.object(appliance, 'STATE', state), patch.object(appliance, 'setup_wifi') as wifi, patch.object(appliance, 'run', return_value=connected) as command, patch('builtins.input', side_effect=['0', '']), patch('builtins.print'):
                appliance.welcome()
                wifi.assert_called_once()
                self.assertTrue((state / 'welcome-complete').is_file())
                command.assert_called_with('systemctl', '--no-block', 'start', 'kanlo-tv.service')

    def test_keyboard_choice_sets_xkb_layout_and_saves_console_map(self):
        with tempfile.TemporaryDirectory() as directory:
            config = Path(directory) / 'keyboard'
            with patch.object(appliance, 'KEYBOARD', config), patch.object(appliance, 'run') as command, patch('builtins.input', return_value='6'), patch('builtins.print'):
                appliance.keyboard_menu()
                command.assert_called_once_with('setupcon', '--keyboard-only', '--force', '--save')
            self.assertIn('XKBLAYOUT="ch"\nXKBVARIANT="fr"', config.read_text())
            with patch.object(appliance, 'KEYBOARD', config), patch.object(appliance, 'run', side_effect=OSError), patch('builtins.input', return_value='1'), patch('builtins.print'), self.assertRaises(OSError):
                appliance.keyboard_menu()
            self.assertIn('XKBLAYOUT="ch"\nXKBVARIANT="fr"', config.read_text())

    def test_stalled_cage_is_stopped_without_a_first_frame(self):
        with tempfile.TemporaryDirectory() as directory:
            marker = Path(directory) / 'first-frame'
            with patch.object(display.subprocess, 'Popen') as spawn, patch.object(display, 'FIRST_FRAME_TIMEOUT', 0), patch('sys.stderr'):
                cage = spawn.return_value.__enter__.return_value
                cage.poll.return_value = None
                self.assertFalse(display.launch_graphical(marker, 'pixman'))
                cage.terminate.assert_called_once()
                self.assertEqual(spawn.call_args.kwargs['env']['WLR_RENDERER'], 'pixman')

    def test_return_to_board_activates_the_display_service_and_console(self):
        with patch.object(appliance, 'run') as command:
            appliance.return_to_board(restart=True)
            self.assertEqual([call.args for call in command.call_args_list], [
                ('systemctl', '--no-block', 'restart', 'kanlo-tv.service'), ('chvt', '1'),
            ])

    def test_welcome_waits_for_pending_wifi_address(self):
        states = [subprocess.CompletedProcess([], 0, state + '\n', '') for state in (
            'wifi:connecting (getting IP configuration)', 'wifi:connected (externally)',
        )]
        with patch.object(appliance, 'run', side_effect=states), patch.object(appliance.time, 'sleep') as wait:
            self.assertTrue(appliance.wifi_ready())
            wait.assert_called_once_with(1)

    def test_settings_menu_routes_to_wifi_and_bluetooth(self):
        with patch.object(appliance, 'wifi_menu') as wifi, patch.object(appliance, 'bluetooth_menu') as bluetooth, patch('builtins.input', side_effect=['1', '2', EOFError]), patch('builtins.print') as output:
            with self.assertRaises(EOFError):
                appliance.setup()
            wifi.assert_called_once()
            bluetooth.assert_called_once()
            self.assertNotIn('Strg+Alt', str(output.call_args_list))
            self.assertNotIn('Strg+Alt', display.setup_hint())

    def test_bluetooth_menu_controls_radio_and_opens_pairing_agent(self):
        with patch.object(appliance, 'run') as command, patch('builtins.input', side_effect=['2', '', '3', '', '4', '', '0']), patch('builtins.print'):
            appliance.bluetooth_menu()
        self.assertEqual([call.args for call in command.call_args_list], [
            ('rfkill', 'unblock', 'bluetooth'),
            ('bluetoothctl', 'power', 'on'),
            ('rfkill', 'block', 'bluetooth'),
            ('bluetoothctl', '--agent', 'KeyboardDisplay'),
        ])

    def test_companion_is_visible_only_with_space_and_before_its_deadline(self):
        now = datetime(2026, 11, 1, 10, 0, tzinfo=timezone.utc)
        active = {'kind': 'ticket', 'ticket_id': 'new'}
        state = {'overlays': [active], 'preferences': {'overlay_minutes': 5},
                 'companion_ticket': {'ticket_id': 'old', 'expires_at': None}}
        self.assertEqual(display.visible_companion(state, active, now, 1920, 1080),
                         {'kind': 'ticket', 'ticket_id': 'old'})
        self.assertIsNone(display.visible_companion(state, active, now, 1024, 600))
        state['companion_ticket']['expires_at'] = now.isoformat()
        self.assertIsNone(display.visible_companion(state, active, now, 1920, 1080))

    @unittest.skipUnless(tv_markdown.PARSER, 'Install python3-markdown-it to test rich Markdown')
    def test_native_markdown_has_table_code_and_inert_image_tokens(self):
        rows = tv_markdown.markdown_rows(
            '# Heading\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n'
            '```js\nalert(1)\n```\n\n- List item\n\n![photo](https://images.example/a.png)\n'
            '<script>throw 1</script>')
        self.assertIn(('heading', 'Heading', None), rows)
        self.assertIn(('table', '1  │  2', None), rows)
        self.assertIn(('code', 'alert(1)', None), rows)
        self.assertIn(('text', '• List item', None), rows)
        self.assertIn(('image', 'photo', 'https://images.example/a.png'), rows)

    def test_external_image_destination_rejects_mixed_and_non_https_addresses(self):
        def endpoints(*addresses):
            return [(None, None, None, None, (address, 443)) for address in addresses]
        with patch.object(tv_markdown.socket, 'getaddrinfo', return_value=endpoints('8.8.8.8', '127.0.0.1')):
            with self.assertRaisesRegex(ValueError, 'non-public'):
                tv_markdown.public_destination('https://images.example/a.png')
        with patch.object(tv_markdown.socket, 'getaddrinfo', return_value=endpoints('8.8.8.8')):
            self.assertEqual(tv_markdown.public_destination('https://images.example/a.png?key=1'),
                             ('images.example', '8.8.8.8', '/a.png?key=1'))
        for url in ('http://images.example/a.png', 'https://user@images.example/a.png',
                    'https://images.example:8080/a.png', 'file:///tmp/image.png'):
            with self.assertRaises(ValueError):
                tv_markdown.public_destination(url)

    def test_external_image_accepts_bounded_png_and_rejects_redirect(self):
        try:
            from PIL import Image
        except ImportError:
            self.skipTest('Install python3-pil to verify decoding')
        png = io.BytesIO()
        Image.new('RGB', (800, 600), '#ab5142').save(png, format='PNG')
        headers = Message()
        headers['Content-Type'] = 'image/png'

        class Response(io.BytesIO):
            status = 200
            length = len(png.getvalue())

        for status in (200, 302):
            response = Response(png.getvalue())
            response.status, response.headers = status, headers
            with patch.object(tv_markdown, 'public_destination', return_value=(
                'images.example', '8.8.8.8', '/image.png'
            )), patch.object(tv_markdown.http.client, 'HTTPSConnection') as connection:
                connection.return_value.getresponse.return_value = response
                if status == 302:
                    with self.assertRaisesRegex(ValueError, 'Invalid image response'):
                        tv_markdown.fetch_image('https://images.example/image.png')
                else:
                    pixels, size = tv_markdown.fetch_image('https://images.example/image.png')
                    self.assertEqual(size, (427, 320))
                    self.assertEqual(len(pixels), 427 * 320 * 3)
    def test_hotspot_uses_private_ephemeral_wpa2_profile_and_removes_it(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            hotspot, profile = root / 'hotspot', root / 'nm' / 'setup.nmconnection'
            commands = []

            def command(*args, **kwargs):
                commands.append(args)
                output = 'wlan0:wifi:disconnected\n' if 'device' in args else ''
                return subprocess.CompletedProcess(args, 0, output, '')

            with patch.object(appliance, 'HOTSPOT', hotspot), patch.object(
                appliance, 'HOTSPOT_PROFILE', profile
            ), patch.object(appliance, 'run', side_effect=command), patch.object(
                appliance.subprocess, 'run', side_effect=lambda args, **kwargs: command(*args)
            ):
                appliance.hotspot_start()
                credentials = json.loads((hotspot / 'credentials.json').read_text())
                contents = profile.read_text()
                self.assertIn('autoconnect=false', contents)
                self.assertIn('proto=rsn;', contents)
                self.assertIn('interface-name=wlan0', contents)
                self.assertIn('psk=' + credentials['password'], contents)
                self.assertNotIn(credentials['password'], repr(commands))
                if sys.platform != 'win32':
                    self.assertEqual(profile.stat().st_mode & 0o777, 0o600)
                appliance.hotspot_stop()
                self.assertFalse(profile.exists())
                self.assertFalse((hotspot / 'credentials.json').exists())
                self.assertTrue(any('delete' in args for args in commands))

    def test_hotspot_does_not_replace_existing_network(self):
        with patch.object(appliance, 'run', return_value=subprocess.CompletedProcess(
            [], 0, '192.168.1.0/24 dev eth0 scope link\n', ''
        )) as command:
            with self.assertRaisesRegex(ValueError, 'LAN connection'):
                appliance.hotspot_start()
            command.assert_called_once()

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
        self.assertEqual(hosts, ['10.0.2.15', '192.168.1.25', 'kanlo-ab12cd', 'kanlo-ab12cd.local'])

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
            ([('card0-HDMI-A-1', 'unknown', ['1920x1080']), ('card1-Composite-1', 'unknown', [])], True),
            ([('card0-HDMI-A-1', 'unknown', []), ('card1-Composite-1', 'unknown', [])], False),
        ]:
            with self.subTest(connectors=connectors), patch.object(appliance, 'display_connectors', return_value=connectors):
                self.assertEqual(appliance.graphical_output(), expected)

    def test_display_launcher_allows_virtual_terminal_switching_and_falls_back(self):
        with (
            patch.object(appliance, 'graphical_output', return_value=True),
            patch.object(display, 'wait_for_seat'),
            patch.object(display, 'launch_graphical', return_value=False) as cage,
            patch.object(display, 'console_board') as fallback,
            patch.dict(display.os.environ, {'XDG_RUNTIME_DIR': '/run/kanlo-display'}),
            patch('sys.stderr'),
        ):
            display.launch()
            self.assertEqual([call.args[1] for call in cage.call_args_list], ['pixman', None])
            fallback.assert_called_once_with()

    def test_console_board_hands_off_to_cage_when_connector_appears(self):
        with (
            patch.object(appliance, 'graphical_output', side_effect=[False, True]) as output,
            patch.object(display, 'wait_for_seat'),
            patch.object(display, 'launch_graphical', return_value=True) as cage,
            patch.object(display, 'console_board') as fallback,
            patch.dict(display.os.environ, {'XDG_RUNTIME_DIR': '/run/kanlo-display'}),
            patch('sys.stderr'),
        ):
            display.launch()
        self.assertEqual(fallback.call_args_list[0].kwargs, {'until': output})
        cage.assert_called_once()
        self.assertFalse(fallback.call_args_list[1].kwargs)

    def test_wifi_menu_starts_local_network_selector_without_credentials_in_command(self):
        with (
            patch('builtins.input', return_value='de'),
            patch.object(appliance, 'set_wifi_country') as country,
            patch.object(appliance, 'run') as run,
            patch.object(appliance, 'prepare_wifi_scan') as scan,
            patch.object(appliance, 'print_web_addresses') as addresses,
        ):
            appliance.setup_wifi()
            country.assert_called_once_with('DE')
            scan.assert_called_once_with()
            self.assertEqual(run.call_args_list[0].args, ('systemctl', 'stop', 'kanlo-hotspot.service'))
            self.assertEqual(run.call_args_list[1].args, ('/usr/bin/nmtui-connect',))
            self.assertEqual(run.call_args_list[2].args, ('systemctl', 'restart', 'kanlo-network.service'))
            addresses.assert_called_once_with()

    def test_first_wifi_scan_waits_for_adapter_after_unblocking(self):
        unavailable = subprocess.CompletedProcess([], 0, 'wifi:unavailable\n', '')
        available = subprocess.CompletedProcess([], 0, 'wifi:disconnected\n', '')
        with (
            patch.object(appliance, 'run', side_effect=[None, None, unavailable, available]) as command,
            patch.object(appliance.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0)) as scan,
            patch.object(appliance.time, 'sleep') as wait,
        ):
            appliance.prepare_wifi_scan()
        self.assertEqual(command.call_args_list[0].args, ('rfkill', 'unblock', 'wifi'))
        self.assertEqual(command.call_args_list[1].args, ('nmcli', '--wait', '5', 'radio', 'wifi', 'on'))
        wait.assert_called_once_with(1)
        self.assertEqual(scan.call_args.args[0][-4:], ['wifi', 'list', '--rescan', 'yes'])

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

    def test_tv_pages_scroll_one_column_at_a_time_before_moving_right(self):
        columns = [({'id': key}, [key + str(index) for index in range(3)])
                   for key in ['A', 'B', 'C']]
        pages = display.paginate_columns(columns, 2, 2)
        self.assertEqual([[tickets for _, tickets, _ in page] for page in pages],
                         [[['A0', 'A1'], ['B0', 'B1']], [['A2'], ['B0', 'B1']],
                          [['A2'], ['B2']], [['C0', 'C1']], [['C2']]])

    def test_tv_overlay_expiry_returns_to_saved_context(self):
        state = {'preferences': {'overlay_minutes': 5},
                 'expires_at': '2026-11-01T10:05:00Z',
                 'overlays': [{'kind': 'backlog', 'board_id': 'a'},
                              {'kind': 'ticket', 'ticket_id': '1'}]}
        self.assertEqual(display.active_overlay(state, datetime(2026, 11, 1, 10, 4, tzinfo=timezone.utc))['kind'], 'ticket')
        self.assertEqual(display.active_overlay(state, datetime(2026, 11, 1, 10, 5, tzinfo=timezone.utc))['kind'], 'backlog')
        self.assertIsNone(display.active_overlay(state, datetime(2026, 11, 1, 10, 10, tzinfo=timezone.utc)))

    def test_deadline_warning_distinguishes_local_date_and_instant(self):
        workspace = {'settings': {'timezone': 'UTC'},
                     'board_options': {'a': {'deadline_warning_days': 0}}}
        ticket = {'board_id': 'a', 'done': False, 'details': {'deadline': '2026-11-01'}}
        now = datetime(2026, 11, 1, 10, 15, tzinfo=timezone.utc)
        self.assertEqual(display.due_warning(ticket, workspace, now)[0], 'Fällig')
        ticket['details']['deadline'] = '2026-11-01T10:00:00Z'
        self.assertEqual(display.due_warning(ticket, workspace, now)[0], 'Überfällig')
        ticket['done'] = True
        self.assertIsNone(display.due_warning(ticket, workspace, now))

    def test_statistics_carry_board_gauges_forward_without_summing_snapshots(self):
        def sample(board, day, created, completed, backlog, blocked):
            return {'board_id': board, 'created_at': day + 'T12:00:00Z',
                    'sample': {'created': created, 'completed': completed, 'backlog': backlog, 'blocked': blocked}}
        samples = [sample('a', '2026-11-01', 1, 0, 1, 0),
                   sample('b', '2026-11-01', 2, 0, 2, 1),
                   sample('a', '2026-11-02', 0, 1, 0, 0)]
        self.assertEqual(display.statistics_series(samples), [('2026-11-01', 3, 0, 3, 1), ('2026-11-02', 0, 1, 2, 1)])

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
