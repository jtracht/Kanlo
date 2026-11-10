"""Native, read-only appliance board display. SDL/Wayland, no browser engine."""
import json
import os
from pathlib import Path
import queue
import threading
import time
import urllib.request


def fetch(path):
    with urllib.request.urlopen('http://127.0.0.1:8080/api/v1/' + path, timeout=4) as response:
        return json.load(response)


def reader(updates, stop):
    revision = None
    while not stop.is_set():
        try:
            bootstrap = fetch('bootstrap')
            identity = (bootstrap.get('instance_id'), bootstrap.get('epoch'), bootstrap['revision'])
            if revision != identity:
                boards = [fetch('boards/' + board['id'] + '/snapshot')['workspace'] for board in bootstrap['boards']]
                updates.put(('boards', boards))
                revision = identity
            updates.put(('online', True))
        except (OSError, ValueError, KeyError):
            updates.put(('online', False))
        stop.wait(3)


def visible(workspace):
    columns = sorted(workspace.get('columns', []), key=lambda c: c['position'])
    if any(t['column_id'] is None and not t['details']['trashed'] and not t['details']['archived'] for t in workspace.get('tickets', [])):
        columns.insert(0, {'id': None, 'name': 'Backlog', 'position': -1})
    return [(column, sorted([t for t in workspace.get('tickets', []) if t['column_id'] == column['id'] and not t['details']['trashed'] and not t['details']['archived']], key=lambda t: t['details']['rank'])) for column in columns]


def paginate(workspace, per_screen, rows):
    columns = visible(workspace)
    pages = []
    for start in range(0, len(columns), per_screen):
        group = columns[start:start + per_screen]
        for offset in range(0, max(1, max(len(tickets) for _, tickets in group)), rows):
            pages.append([(column, tickets[offset:offset + rows], len(tickets)) for column, tickets in group])
    return pages


def main():
    import pygame
    pygame.display.init()
    pygame.font.init()
    pygame.mouse.set_visible(False)
    desktop = pygame.display.get_desktop_sizes()[0]
    # Cage owns fullscreen; an SDL exclusive-mode request can block on Wayland.
    screen = pygame.display.set_mode(desktop, pygame.RESIZABLE)
    pygame.display.set_caption('Kanlo')
    width, height = screen.get_size()
    scale = max(0.65, min(width / 1600, height / 900))
    font_path = '/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'
    font = pygame.font.Font(font_path, round(24 * scale))
    title_font = pygame.font.Font(font_path, round(38 * scale))
    small = pygame.font.Font(font_path, round(18 * scale))
    updates = queue.Queue(maxsize=0)
    stop = threading.Event()
    threading.Thread(target=reader, args=(updates, stop), daemon=True).start()
    boards, active, page, page_count, online = [], 0, 0, 1, False
    dirty, last_rotate, last_urls = True, time.monotonic(), ''
    def text(value, position, face=font, color='#e5e7eb', max_width=None):
        value = str(value)
        if max_width:
            while value and face.size(value)[0] > max_width:
                value = value[:-2] + '…' if not value.endswith('…') else value[:-2] + '…'
                if len(value) <= 1:
                    break
        screen.blit(face.render(value, True, color), position)
    try:
        while not stop.is_set():
            # An event-driven, capped loop: repaint only on data/page/connection changes.
            event = pygame.event.wait(250)
            if event.type == pygame.QUIT or event.type == pygame.KEYDOWN and event.key == pygame.K_ESCAPE:
                break
            if event.type == pygame.KEYDOWN and event.key in (pygame.K_RIGHT, pygame.K_SPACE):
                active += 1; page = 0; dirty = True
            while not updates.empty():
                kind, value = updates.get_nowait()
                if kind == 'boards':
                    boards = value; dirty = True
                elif online != value:
                    online = value; dirty = True
            try:
                urls = '  '.join(json.loads(Path('/run/kanlo-network/urls.json').read_text()))
            except FileNotFoundError:
                urls = 'Ethernet anschließen / Ctrl+Alt+F2: Einrichtung'
            if urls != last_urls:
                last_urls = urls; dirty = True
            if time.monotonic() - last_rotate > 20:
                page += 1
                if page >= page_count:
                    page = 0; active += 1
                dirty = True; last_rotate = time.monotonic()
            if not dirty:
                continue
            dirty = False
            screen.fill('#101820')
            margin = round(28 * scale)
            text('Kanlo', (margin, margin), title_font)
            text(urls, (margin, height - 2 * margin), small, max_width=width - 2 * margin)
            text('Ctrl+Alt+F2: Einrichtung / setup' if online else 'Verbindung wird wiederhergestellt …', (margin, height - 3 * margin), small, '#9ca3af')
            if not boards:
                text('Bereit – Board über die Weboberfläche einrichten.', (margin, height // 2), max_width=width - 2 * margin)
            else:
                active %= len(boards)
                workspace = boards[active]
                board = workspace['boards'][0]
                text(board['name'], (round(190 * scale), margin), title_font, max_width=width - round(230 * scale))
                columns = visible(workspace)
                page_count = 1
                if columns:
                    per_screen = max(1, min(len(columns), width // round(280 * scale)))
                    column_width = (width - margin * 2) // per_screen
                    card_height = round(100 * scale)
                    rows = max(1, (height - round(210 * scale)) // card_height)
                    pages = paginate(workspace, per_screen, rows)
                    page_count = len(pages)
                    selected = pages[page % page_count]
                    for index, (column, tickets, total) in enumerate(selected):
                        x = margin + index * column_width
                        text(f"{column['name']} · {total}", (x, round(100 * scale)), max_width=column_width - margin)
                        for row, ticket in enumerate(tickets):
                            y = round(150 * scale) + row * card_height
                            pygame.draw.rect(screen, '#22313d', (x, y, column_width - 12, card_height - 10), border_radius=8)
                            text(ticket['public_id'], (x + 10, y + 8), small, '#67e8c9')
                            text(ticket['title'], (x + 10, y + round(38 * scale)), max_width=column_width - 32)
            pygame.display.flip()
            if os.environ.get('KANLO_DISPLAY_TEST') == '1':
                pygame.image.save(screen, os.environ.get('KANLO_DISPLAY_SCREENSHOT', '/tmp/kanlo-display.png'))
                break
    finally:
        stop.set()
        pygame.quit()


if __name__ == '__main__':
    main()
