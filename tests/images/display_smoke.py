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
             'tickets': [{'id': 'ticket', 'public_id': 'IMG-1', 'column_id': 'todo', 'title': 'Native SDL/Wayland · Überprüfung', 'details': {'trashed': False, 'archived': False, 'rank': 1}}]}
def provide_board(updates, stop):
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
        assert surface.get_at((margin + 12, round(130 * scale))) != surface.get_at((margin + 12, round(225 * scale)))
        print(f'PASS: native board and ticket at {width}x{height}')
peak_mib = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024
limit = 100 if '--formats' in sys.argv else 192
assert peak_mib < limit, f'Native display exceeded {limit} MiB: {peak_mib:.1f} MiB'
print(f'PASS: native renderer peak {peak_mib:.1f} MiB')
