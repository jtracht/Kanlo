"""Exercise the actual SDL/Wayland renderer with representative board data."""
import os
import resource
from pathlib import Path
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'packaging/images'))
import display

workspace = {'boards': [{'id': 'board', 'name': 'Kanlo appliance acceptance'}],
             'columns': [{'id': 'todo', 'name': 'Offen', 'position': 0}],
             'tickets': [{'id': 'ticket', 'public_id': 'IMG-1', 'column_id': 'todo',
                          'title': 'Native SDL/Wayland · Überprüfung',
                          'description': '# Beschreibung\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n```sh\necho test\n```',
                          'details': {'trashed': False, 'archived': False, 'rank': 1}},
                         {'id': 'companion', 'public_id': 'IMG-2', 'column_id': 'todo',
                          'title': 'Zweite Einblendung', 'description': 'Ein zweites Ticket',
                          'details': {'trashed': False, 'archived': False, 'rank': 2}}]}

current = {'view': 'board', 'overlay': None}
def provide_board(updates, stop, display_updates=None):
    if display_updates is not None:
        display_updates.put({'revision': 1, 'preferences': {'view': current['view']},
                             'overlays': [current['overlay']] if current['overlay'] else [],
                             'companion_ticket': current.get('companion'),
                             'overview': [{'name': 'Kanlo', 'open': 1, 'done': 0, 'backlog': 0, 'blocked': 0}],
                             'statistics': [('2026-11-01', 1, 0, 1, 0)]})
    updates.put(([workspace], True))


display.reader = provide_board
os.environ['KANLO_DISPLAY_TEST'] = '1'
with tempfile.TemporaryDirectory() as directory:
    screenshot = Path(directory) / 'display.png'
    os.environ['KANLO_DISPLAY_SCREENSHOT'] = str(screenshot)
    for size in [(720, 576), (720, 1280), (1920, 1080), (2560, 1080)] if '--formats' in sys.argv else [None]:
        display.main(size)
        import pygame
        surface = pygame.image.load(str(screenshot))
        if size:
            assert surface.get_size() == size
        width, height = surface.get_size()
        assert width >= 640 and height >= 480
        assert screenshot.stat().st_size > 3000
        scale, margin, _, _, _ = display.layout_metrics(width, height)
        x = margin + round(30 * scale)
        assert surface.get_at((x, round(130 * scale)))[:3] == pygame.Color(display.PANEL)[:3]
        assert surface.get_at((x, round(215 * scale)))[:3] == pygame.Color(display.CARD)[:3]
        print(f'PASS: native board and ticket at {width}x{height}')
    if '--formats' in sys.argv:
        for mode, overlay in [('overview', None), ('statistics', None),
                              ('board', {'kind': 'ticket', 'ticket_id': 'ticket'}),
                              ('board', {'kind': 'backlog', 'board_id': 'board'}),
                              ('board', {'kind': 'children', 'ticket_id': 'ticket'})]:
            current.update(view=mode, overlay=overlay)
            display.main((720, 576))
            assert screenshot.stat().st_size > 3000
            print(f'PASS: native {overlay["kind"] if overlay else mode} at 720x576')
        current.update(overlay={'kind': 'ticket', 'ticket_id': 'ticket'},
                       companion={'ticket_id': 'companion', 'expires_at': None})
        for size, expected in [((720, 576), True), ((1920, 1080), False)]:
            display.main(size)
            surface = pygame.image.load(str(screenshot))
            center_color = surface.get_at((size[0] // 2, size[1] // 2))[:3]
            assert (center_color == pygame.Color(display.PANEL)[:3]) == expected
            print(f'PASS: native overlay layout at {size[0]}x{size[1]}')
peak_mib = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024
limit = 100 if '--formats' in sys.argv else 192
assert peak_mib < limit, f'Native display exceeded {limit} MiB: {peak_mib:.1f} MiB'
print(f'PASS: native renderer peak {peak_mib:.1f} MiB')
