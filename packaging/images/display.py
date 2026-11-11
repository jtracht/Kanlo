"""Native, read-only appliance board display. SDL/Wayland, no browser engine."""
import argparse
from http.client import HTTPException
import json
import os
from pathlib import Path
import queue
import shutil
import socket
import subprocess
import sys
import threading
import time
import urllib.request


def fetch(path):
    with urllib.request.urlopen('http://127.0.0.1:8080/api/v1/' + path, timeout=4) as response:
        return json.load(response)


def event_changes(response):
    event, data = None, []
    for raw in response:
        line = raw.decode('utf-8').rstrip('\r\n')
        if not line:
            if event == 'change' and data:
                yield json.loads('\n'.join(data))
            event, data = None, []
        elif line.startswith('event:'):
            event = line.removeprefix('event:').strip()
        elif line.startswith('data:'):
            data.append(line.removeprefix('data:').lstrip(' '))


def reader(updates, stop):
    cache = {}
    last_state = None

    def publish(online):
        nonlocal last_state
        state = list(cache.values()), online
        if state == last_state:
            return
        last_state = state
        try:
            updates.get_nowait()
        except queue.Empty:
            pass
        updates.put_nowait(state)

    def refresh(bootstrap, affected):
        nonlocal cache
        next_cache = {}
        for board in bootstrap['boards']:
            key = board['id']
            if affected is None or key in affected or key not in cache:
                next_cache[key] = fetch(f'boards/{key}/snapshot')['workspace']
            else:
                next_cache[key] = cache[key]
        cache = next_cache

    while not stop.is_set():
        try:
            bootstrap = fetch('bootstrap')
            identity = bootstrap['instance_id'], bootstrap['epoch']
            refresh(bootstrap, None)
            publish(True)
            with urllib.request.urlopen(
                f'http://127.0.0.1:8080/api/v1/events?after={bootstrap["event_cursor"]}',
                timeout=22,
            ) as response:
                for change in event_changes(response):
                    if stop.is_set():
                        return
                    bootstrap = fetch('bootstrap')
                    current = bootstrap['instance_id'], bootstrap['epoch']
                    refresh(bootstrap, None if current != identity or change.get('restored') else set(change['boards']))
                    identity = current
                    publish(True)
            publish(False)
        except (OSError, ValueError, KeyError, HTTPException):
            publish(False)
        stop.wait(2)


def visible(workspace):
    columns = sorted(workspace.get('columns', []), key=lambda c: c['position'])
    grouped = {}
    for ticket in workspace.get('tickets', []):
        if not ticket['details']['trashed'] and not ticket['details']['archived']:
            grouped.setdefault(ticket['column_id'], []).append(ticket)
    if None in grouped:
        columns.insert(0, {'id': None, 'name': 'Backlog', 'position': -1})
    return [(column, sorted(grouped.get(column['id'], []), key=lambda t: t['details']['rank'])) for column in columns]


def paginate_columns(columns, per_screen, rows):
    pages = []
    for start in range(0, len(columns), per_screen):
        group = columns[start:start + per_screen]
        for offset in range(0, max(1, max(len(tickets) for _, tickets in group)), rows):
            pages.append([(column, tickets[offset:offset + rows], len(tickets)) for column, tickets in group])
    return pages


def paginate(workspace, per_screen, rows):
    return paginate_columns(visible(workspace), per_screen, rows)


def layout_metrics(width, height):
    scale = max(0.6, min(height / 900, width / 960, 1.75))
    margin = round(28 * scale)
    card = round(100 * scale)
    columns = max(1, (width - 2 * margin) // round(280 * scale))
    rows = max(1, (height - round(230 * scale)) // card)
    return scale, margin, card, columns, rows


def console_board():
    updates = queue.Queue(maxsize=1)
    stop = threading.Event()
    threading.Thread(target=reader, args=(updates, stop), daemon=True).start()
    boards, online, previous_frame = [], False, None
    while True:
        try:
            boards, online = updates.get(timeout=1)
        except queue.Empty:
            pass
        width, height = shutil.get_terminal_size((80, 25))
        lines = ['Kanlo · Board-Anzeige', 'Strg+Alt+F2: WLAN und Einrichtung', '']
        if not online:
            lines.append('Kanlo wartet auf den Server.')
        urls = web_addresses()
        if urls:
            lines.append('Web: ' + urls)
        if boards:
            workspace = boards[int(time.monotonic() // 20) % len(boards)]
            lines.extend([workspace['boards'][0]['name'], ''])
            for column, tickets in visible(workspace):
                lines.append(f"{column['name']} · {len(tickets)}")
                lines.extend(f"  {ticket['public_id']}  {ticket['title']}" for ticket in tickets[:3])
        # Ticket text is data, including on terminals that interpret escape sequences.
        safe = [''.join(char if char.isprintable() else ' ' for char in line)[:max(1, width - 1)]
                for line in lines]
        frame = '\n'.join(safe[:max(1, height - 1)])
        if frame != previous_frame:
            print('\033[2J\033[H' + frame, flush=True)
            previous_frame = frame


def web_addresses():
    try:
        return '  '.join(json.loads(Path('/run/kanlo-network/urls.json').read_text()))
    except (OSError, ValueError):
        return ''


def launch():
    from appliance import graphical_output

    if graphical_output():
        try:
            wait_for_seat()
            result = subprocess.run(['/usr/bin/cage', '-s', '--', sys.executable, __file__])
            print(f'Kanlo graphical display exited ({result.returncode}); using console board.', file=sys.stderr)
        except OSError as error:
            print(f'Kanlo graphical display could not start: {error}; using console board.', file=sys.stderr)
    console_board()


def wait_for_seat():
    # Debian's seatd service is Type=simple. Its socket can appear after systemd
    # has already released the ordering dependency for the display service.
    for attempt in range(20):
        try:
            with socket.socket(socket.AF_UNIX) as connection:
                connection.settimeout(0.25)
                connection.connect('/run/seatd.sock')
                return
        except OSError:
            if attempt == 19:
                raise
            time.sleep(0.25)


def main(size=None):
    import pygame
    pygame.display.init()
    pygame.font.init()
    pygame.mouse.set_visible(False)
    desktop = size or pygame.display.get_desktop_sizes()[0]
    # Cage owns fullscreen; an SDL exclusive-mode request can block on Wayland.
    screen = pygame.display.set_mode(desktop, pygame.RESIZABLE)
    pygame.display.set_caption('Kanlo')
    font_path = '/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'

    def layout():
        width, height = screen.get_size()
        scale, margin, card, columns, rows = layout_metrics(width, height)
        fonts = [pygame.font.Font(font_path, round(size * scale)) for size in (24, 38, 18)]
        return width, height, scale, margin, card, columns, rows, *fonts

    width, height, scale, margin, card_height, per_screen, rows, font, title_font, small = layout()
    updates = queue.Queue(maxsize=1)
    stop = threading.Event()
    threading.Thread(target=reader, args=(updates, stop), daemon=True).start()
    boards, active, page, page_count, online = [], 0, 0, 1, False
    dirty, footer_dirty, last_rotate, last_urls = True, True, time.monotonic(), ''
    def text(value, position, face=None, color='#e5e7eb', max_width=None):
        face = font if face is None else face
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
            if screen.get_size() != (width, height):
                width, height, scale, margin, card_height, per_screen, rows, font, title_font, small = layout()
                dirty = True
                footer_dirty = True
            if event.type == pygame.QUIT or event.type == pygame.KEYDOWN and event.key == pygame.K_ESCAPE:
                break
            if event.type == pygame.KEYDOWN and event.key in (pygame.K_RIGHT, pygame.K_SPACE):
                active += 1; page = 0; dirty = True
            try:
                new_boards, new_online = updates.get_nowait()
                dirty |= boards != new_boards
                footer_dirty |= online != new_online
                boards, online = new_boards, new_online
            except queue.Empty:
                pass
            urls = web_addresses() or 'Strg+Alt+F2: WLAN einrichten oder Ethernet anschließen'
            if urls != last_urls:
                last_urls = urls
                footer_dirty = True
            if time.monotonic() - last_rotate > 20:
                page += 1
                if page >= page_count:
                    page = 0; active += 1
                dirty = True; last_rotate = time.monotonic()
            if not dirty and not footer_dirty:
                continue
            footer = pygame.Rect(0, height - 4 * margin, width, 4 * margin)
            if footer_dirty:
                screen.fill('#101820', footer)
                text(urls, (margin, height - 2 * margin), small, max_width=width - 2 * margin)
                text('Strg+Alt+F2: WLAN und Einrichtung' if online else 'Verbindung wird wiederhergestellt …', (margin, height - 3 * margin), small, '#9ca3af')
                footer_dirty = False
            if not dirty:
                pygame.display.update(footer)
                continue
            dirty = False
            screen.fill('#101820', pygame.Rect(0, 0, width, footer.top))
            text('Kanlo', (margin, margin), title_font)
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
                    shown_columns = min(len(columns), per_screen)
                    column_width = (width - margin * 2) // shown_columns
                    pages = paginate_columns(columns, shown_columns, rows)
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
            if os.environ.get('KANLO_DISPLAY_TEST') == '1' and boards:
                pygame.image.save(screen, os.environ.get('KANLO_DISPLAY_SCREENSHOT', '/tmp/kanlo-display.png'))
                break
    finally:
        stop.set()
        pygame.quit()


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--launch', action='store_true')
    if parser.parse_args().launch:
        launch()
    else:
        main()
