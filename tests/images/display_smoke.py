"""Exercise the actual SDL/Wayland renderer with representative board data."""
import os
from pathlib import Path
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'packaging/images'))
import display

workspace = {'boards': [{'id': 'board', 'name': 'Kanlo appliance acceptance'}],
             'columns': [{'id': 'todo', 'name': 'Offen', 'position': 0}],
             'tickets': [{'id': 'ticket', 'public_id': 'IMG-1', 'column_id': 'todo', 'title': 'Native SDL/Wayland · Überprüfung', 'details': {'trashed': False, 'archived': False, 'rank': 1}}]}
display.fetch = lambda path: {'revision': 1, 'boards': workspace['boards']} if path == 'bootstrap' else {'workspace': workspace}
os.environ['KANLO_DISPLAY_TEST'] = '1'
with tempfile.TemporaryDirectory() as directory:
    screenshot = Path(directory) / 'display.png'
    os.environ['KANLO_DISPLAY_SCREENSHOT'] = str(screenshot)
    display.main()
    import pygame
    surface = pygame.image.load(str(screenshot))
    assert surface.get_width() >= 640 and surface.get_height() >= 480
    assert screenshot.stat().st_size > 3000
print('PASS: native SDL renderer on a real Wayland compositor, board/ticket, no network')
