"""Native, read-only appliance board display. SDL/Wayland, no browser engine."""
import argparse
from datetime import datetime, timedelta, timezone
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
import textwrap
import urllib.parse
import urllib.request
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError
from tv_markdown import Images, markdown_rows

SETUP_AVAILABLE = Path('/etc/systemd/system/kanlo-setup.service')
RUNTIME_VIEW = Path('/var/lib/kanlo-display/view.json')
ANIMATIONS = {'off': 0, 'subtle': 0.10, 'fluid': 0.20, 'dynamic': 0.32}


def fetch(path):
    with urllib.request.urlopen('http://127.0.0.1:8080/api/v1/' + path, timeout=4) as response:
        return json.load(response)


def resume_timer(revision):
    origin = 'http://127.0.0.1:8080'
    request = urllib.request.Request(origin + '/api/v1/display',
        data=json.dumps({'kind': 'resume', 'revision': revision}).encode(),
        headers={'Origin': origin, 'Content-Type': 'application/json'})
    with urllib.request.urlopen(request, timeout=4) as response:
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


def offer(mailbox, state):
    try:
        mailbox.get_nowait()
    except queue.Empty:
        pass
    mailbox.put_nowait(state)


def reader(updates, stop, display_updates=None):
    cache = {}
    last_state = None
    saved = read_runtime_view() if display_updates is not None else None
    resumed = None

    def publish(online):
        nonlocal last_state
        state = list(cache.values()), online
        if state == last_state:
            return
        last_state = state
        offer(updates, state)

    def refresh_display(bootstrap):
        nonlocal saved, resumed
        if display_updates is None:
            return
        state = fetch('display')
        instance = [bootstrap['instance_id'], bootstrap['epoch']]
        if saved is not None:
            if state['overlays'] and state['expires_at'] and saved.get('revision') == state['revision'] and saved.get('instance') == instance:
                previous = state['revision']
                try:
                    state = resume_timer(previous)
                    resumed = state['revision'], previous
                except (OSError, ValueError, HTTPException):
                    # A concurrent control action wins over restoring an older timer.
                    state = fetch('display')
            saved = None
        if resumed and resumed[0] == state['revision']:
            state['resumed_revision'] = resumed[1]
        state['animation'] = bootstrap.get('settings', {}).get('animation', 'fluid')
        state['instance'] = instance
        preferences = state['preferences']
        modes = {entry['view'] for entry in preferences['rotation']} | {preferences['view']}
        if 'overview' in modes:
            state['overview'] = fetch('overview')
        if 'statistics' in modes:
            now = datetime.now(timezone.utc)
            query = {'from': (now - timedelta(days=preferences.get('statistics_days', 7))).isoformat(timespec='milliseconds'),
                     'to': now.isoformat(timespec='milliseconds')}
            if preferences['board_id']:
                query['board_id'] = preferences['board_id']
            state['statistics'] = statistics_series(fetch('statistics?' + urllib.parse.urlencode(query)))
        offer(display_updates, state)

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
            refresh_display(bootstrap)
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
                    refresh_display(bootstrap)
            publish(False)
        except (OSError, ValueError, KeyError, HTTPException):
            publish(False)
        stop.wait(2)


def statistics_series(samples):
    days = {}
    for entry in samples:
        day = entry['created_at'][:10]
        counts = days.setdefault(day, [0, 0, {}])
        sample = entry['sample']
        counts[0] += sample['created']
        counts[1] += sample['completed']
        counts[2][entry['board_id']] = (sample['backlog'], sample['blocked'])
    latest = {}
    points = []
    for day, (created, completed, board_counts) in sorted(days.items()):
        latest.update(board_counts)
        points.append((day, created, completed, sum(value[0] for value in latest.values()),
                       sum(value[1] for value in latest.values())))
    size = max(1, (len(points) + 6) // 7)
    grouped = []
    for offset in range(0, len(points), size):
        batch = points[offset:offset + size]
        day, _, _, backlog, blocked = batch[-1]
        grouped.append((day, sum(item[1] for item in batch), sum(item[2] for item in batch), backlog, blocked))
    return grouped


def due_warning(ticket, workspace, now):
    value = ticket['details'].get('deadline')
    if not value or ticket['done']:
        return None
    board_id = ticket['board_id']
    days = workspace.get('board_options', {}).get(board_id, {}).get('deadline_warning_days', 3)
    try:
        zone = workspace.get('settings', {}).get('timezone', 'UTC')
        local = now.astimezone(timezone.utc if zone == 'UTC' else ZoneInfo(zone))
        if 'T' in value:
            deadline = datetime.fromisoformat(value)
            if deadline <= now:
                return 'Überfällig', '#f09098'
            remaining = (deadline.astimezone(local.tzinfo).date() - local.date()).days
        else:
            remaining = (datetime.strptime(value, '%Y-%m-%d').date() - local.date()).days
            if remaining < 0:
                return 'Überfällig', '#f09098'
    except (ValueError, ZoneInfoNotFoundError):
        return None
    return ('Fällig', '#e7bf82') if remaining <= days else None


def display_content(boards, preferences, overlay, mode):
    if not boards:
        return '', []
    board_id = (overlay or {}).get('board_id') or preferences.get('board_id')
    board = next((board for board in boards if board['boards'][0]['id'] == board_id), boards[0])
    if overlay:
        if overlay['kind'] == 'backlog':
            return 'Backlog · ' + board['boards'][0]['name'], [
                (ticket['public_id'], ticket['title']) for column, tickets in visible(board)
                if column['id'] is None for ticket in tickets
            ]
        ticket_id = overlay['ticket_id']
        for workspace in boards:
            ticket = next((ticket for ticket in workspace['tickets'] if ticket['id'] == ticket_id
                           and not ticket['details']['archived'] and not ticket['details']['trashed']), None)
            if ticket is None:
                continue
            if overlay['kind'] == 'children':
                return 'Untertickets · ' + ticket['public_id'], [
                    (child['public_id'], child['title']) for child in workspace['tickets']
                    if child['details'].get('parent_id') == ticket_id and not child['details']['trashed']
                    and not child['details']['archived']
                ]
            details = [(None, ticket['description'])]
            comments = sorted((comment for comment in workspace.get('comments', [])
                               if comment['ticket_id'] == ticket_id), key=lambda comment: comment['created_at'])
            details.extend(('Kommentar', comment['markdown']) for comment in comments)
            by_id = {item['id']: item['public_id'] for other in boards for item in other['tickets']}
            details.extend(('Verknüpft', by_id[link['target_id']]) for link in workspace.get('links', [])
                           if link['ticket_id'] == ticket_id and link['target_id'] in by_id)
            return ticket['public_id'] + ' · ' + ticket['title'], details
        return 'Ticket nicht verfügbar', []
    return board['boards'][0]['name'], []


def local_overlays(state, now):
    overlays = state.get('overlays', [])
    if not overlays:
        return None, None
    default = state.get('preferences', {}).get('overlay_minutes', 5)
    overrides = state.get('ticket_minutes', {})
    expires = datetime.fromisoformat(state['expires_at']) if state.get('expires_at') else None
    companion = state.get('companion_ticket')
    side = {'kind': 'ticket', 'ticket_id': companion['ticket_id']} if companion else None
    side_expires = datetime.fromisoformat(companion['expires_at']) if companion and companion['expires_at'] else None
    for index in range(len(overlays) - 1, -1, -1):
        overlay = overlays[index]
        if expires is None or expires > now:
            return overlay, side if index == 0 and (side_expires is None or side_expires > now) else None
        if index > 0:
            restored = overlays[index - 1]
            minutes = overrides.get(restored.get('ticket_id'), default) if restored['kind'] == 'ticket' else default
            if index == 1 and side:
                side_minutes = overrides.get(side['ticket_id'], default)
                side_expires = expires + timedelta(minutes=side_minutes) if side_minutes else None
            expires = expires + timedelta(minutes=minutes) if minutes else None
    return (side if side_expires is None or side_expires > now else None), None


def active_overlay(state, now):
    return local_overlays(state, now)[0]


def visible_companion(state, active, now, width, height):
    if not active or width < 1152 or height < 600:
        return None
    return local_overlays(state, now)[1]


def read_runtime_view():
    try:
        state = json.loads(RUNTIME_VIEW.read_text())
        return state if isinstance(state, dict) else {}
    except (OSError, ValueError):
        return {}


def write_runtime_view(state, mode, page, overlay, overlay_page, rotation_index, companion_page):
    from appliance import atomic

    if not RUNTIME_VIEW.parent.is_dir():
        return
    data = {'instance': state['instance'], 'revision': state['revision'], 'mode': mode,
            'page': page, 'overlay': overlay, 'overlay_page': overlay_page,
            'rotation_index': rotation_index, 'companion_page': companion_page}
    try:
        atomic(RUNTIME_VIEW, json.dumps(data))
    except OSError as error:
        print(f'Could not save TV position: {error}', file=sys.stderr)


def visible(workspace):
    columns = sorted(workspace.get('columns', []), key=lambda c: c['position'])
    grouped = {}
    for ticket in workspace.get('tickets', []):
        if not ticket['details']['trashed'] and not ticket['details']['archived']:
            grouped.setdefault(ticket['column_id'], []).append(ticket)
    if None in grouped:
        columns.insert(0, {'id': None, 'name': 'Backlog', 'position': -1})
    result = []
    for column in columns:
        tickets = sorted(grouped.get(column['id'], []), key=lambda ticket: ticket['details']['rank'])
        root_count = sum(ticket['details'].get('parent_id') is None for ticket in tickets)
        result.append(({**column, 'root_count': root_count}, tickets))
    return result


def paginate_columns(columns, per_screen, rows):
    pages = []
    for start in range(0, len(columns), per_screen):
        group = columns[start:start + per_screen]
        offsets = [0] * len(group)

        def page():
            return [(column, tickets[offset:offset + rows], len(tickets))
                    for (column, tickets), offset in zip(group, offsets)]

        pages.append(page())
        for index, (_, tickets) in enumerate(group):
            for offset in range(rows, len(tickets), rows):
                offsets[index] = offset
                pages.append(page())
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
        lines = ['Kanlo · Board-Anzeige', setup_hint(), '']
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
        return '' if SETUP_AVAILABLE.is_file() else 'http://127.0.0.1:8080'


def setup_hint():
    return 'Strg+Alt+F2: WLAN und Einrichtung' if SETUP_AVAILABLE.is_file() else 'TV-Anzeige über Kanlo im Browser steuern'


def setup_qr(url):
    if not url.startswith('http://') or len(url) > 100:
        return []
    try:
        import qrcode
    except ImportError:
        return []
    code = qrcode.QRCode(border=4)
    code.add_data(url)
    code.make(fit=True)
    return code.get_matrix()


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
    images = Images(os.environ.get('KANLO_TV_EXTERNAL_IMAGES') == '1')
    updates = queue.Queue(maxsize=1)
    display_updates = queue.Queue(maxsize=1)
    stop = threading.Event()
    threading.Thread(target=reader, args=(updates, stop, display_updates), daemon=True).start()
    boards, page, page_count, online = [], 0, 1, False
    dirty, footer_dirty, last_rotate, last_urls = True, True, time.monotonic(), ''
    qr_matrix = []
    last_scroll = last_rotate
    state, preferences, mode, overlay = {}, {}, 'board', None
    companion = None
    overlay_page, board_page_before_overlay = 0, 0
    overlay_pages = {}
    highlights = set()
    focus_highlight = False
    last_minute = None
    saved = read_runtime_view()
    last_saved = None
    transition = None
    animate = False
    rotation_index = -1
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
            event = pygame.event.wait(33 if transition else 250)
            dirty |= images.ready()
            if screen.get_size() != (width, height):
                width, height, scale, margin, card_height, per_screen, rows, font, title_font, small = layout()
                dirty = True
                footer_dirty = True
            if event.type == pygame.QUIT or event.type == pygame.KEYDOWN and event.key == pygame.K_ESCAPE:
                break
            if event.type == pygame.KEYDOWN and event.key in (pygame.K_RIGHT, pygame.K_LEFT, pygame.K_SPACE):
                direction = -1 if event.key == pygame.K_LEFT else 1
                if overlay:
                    overlay_page = max(0, overlay_page + direction)
                else:
                    page = (page + direction) % page_count
                last_scroll = time.monotonic()
                dirty = True
            try:
                new_boards, new_online = updates.get_nowait()
                dirty |= boards != new_boards
                footer_dirty |= online != new_online
                boards, online = new_boards, new_online
            except queue.Empty:
                pass
            try:
                new_state = display_updates.get_nowait()
                if state != new_state:
                    new_preferences = new_state['preferences']
                    if preferences != new_preferences:
                        mode, page, last_rotate = new_preferences['view'], 0, time.monotonic()
                        preferences = new_preferences
                        animate = bool(state)
                        rotation_index = -1
                    if not state and saved.get('instance') == new_state.get('instance') and saved.get('revision') in (new_state['revision'], new_state.get('resumed_revision')):
                        if saved.get('mode') in ('board', 'overview', 'statistics'):
                            mode = saved['mode']
                        if isinstance(saved.get('page'), int):
                            page = max(0, saved['page'])
                        if isinstance(saved.get('rotation_index'), int) and 0 <= saved['rotation_index'] < len(preferences['rotation']):
                            rotation_index = saved['rotation_index']
                        if isinstance(saved.get('overlay'), dict) and isinstance(saved.get('overlay_page'), int):
                            overlay_pages[tuple(saved['overlay'].items())] = max(0, saved['overlay_page'])
                        if isinstance(saved.get('companion_page'), list) and len(saved['companion_page']) == 2:
                            saved_id, saved_page = saved['companion_page']
                            if isinstance(saved_id, str) and isinstance(saved_page, int):
                                overlay_pages[(('kind', 'ticket'), ('ticket_id', saved_id))] = max(0, saved_page)
                    scroll = new_state.get('scroll_offset', 0) - state.get('scroll_offset', 0)
                    if state and scroll:
                        if overlay:
                            overlay_page = max(0, overlay_page + scroll)
                        else:
                            page = (page + scroll) % page_count
                        last_scroll = time.monotonic()
                    state, dirty = new_state, True
            except queue.Empty:
                pass
            now = datetime.now(timezone.utc)
            minute = int(now.timestamp() // 60)
            if minute != last_minute:
                dirty, last_minute = True, minute
            next_overlay = active_overlay(state, now)
            next_companion = visible_companion(state, next_overlay, now, width, height)
            if next_companion != companion:
                companion, dirty = next_companion, True
            next_highlights = set(state.get('highlights', []))
            if state.get('highlights_expire_at') and datetime.fromisoformat(state['highlights_expire_at']) <= now:
                next_highlights.clear()
            if next_highlights != highlights:
                highlights = next_highlights
                dirty = True
                focus_highlight = bool(highlights)
            if next_overlay != overlay:
                if not overlay:
                    board_page_before_overlay = page
                    if last_saved is not None:
                        overlay_pages.clear()
                else:
                    overlay_pages[tuple(overlay.items())] = overlay_page
                if overlay and not next_overlay:
                    page = board_page_before_overlay
                overlay = next_overlay
                overlay_page = overlay_pages.get(tuple(overlay.items()), 0) if overlay else 0
                dirty = True
                animate = True
                last_rotate = time.monotonic()
                last_scroll = last_rotate
            urls = web_addresses() or 'Strg+Alt+F2: WLAN einrichten oder Ethernet anschließen'
            if urls != last_urls:
                last_urls = urls
                qr_matrix = setup_qr(urls.split()[0]) if SETUP_AVAILABLE.is_file() else []
                dirty = True
                footer_dirty = True
            rotation = preferences.get('rotation', [])
            if not overlay and rotation and time.monotonic() - last_rotate >= preferences['rotation_seconds']:
                rotation_index = (rotation_index + 1) % len(rotation)
                mode, page, dirty = rotation[rotation_index]['view'], 0, True
                animate = True
                last_rotate = time.monotonic()
            scroll_seconds = preferences.get('auto_scroll_seconds', 0)
            if not overlay and mode == 'board' and scroll_seconds and time.monotonic() - last_scroll >= scroll_seconds:
                page, dirty = (page + 1) % page_count, True
                last_scroll = time.monotonic()
            if transition and not dirty and not footer_dirty:
                previous, target, start, duration = transition
                fraction = min(1, (time.monotonic() - start) / duration)
                screen.blit(previous, (0, 0))
                target.set_alpha(round(255 * fraction))
                screen.blit(target, (0, 0))
                pygame.display.flip()
                if fraction >= 1:
                    transition = None
                continue
            if not dirty and not footer_dirty:
                continue
            if transition and not dirty:
                target = transition[1]
                target.set_alpha(None)
                screen.blit(target, (0, 0))
            transition = None
            duration = ANIMATIONS.get(state.get('animation'), 0)
            previous = screen.copy() if animate and duration else None
            animate = False
            footer = pygame.Rect(0, height - 4 * margin, width, 4 * margin)
            if footer_dirty:
                screen.fill('#101820', footer)
                text(urls, (margin, height - 2 * margin), small, max_width=width - 2 * margin)
                text(setup_hint() if online else 'Verbindung wird wiederhergestellt …', (margin, height - 3 * margin), small, '#9ca3af')
                footer_dirty = False
            if not dirty:
                pygame.display.update(footer)
                continue
            dirty = False
            screen.fill('#101820', pygame.Rect(0, 0, width, footer.top))
            text('Kanlo', (margin, margin), title_font)
            if not boards:
                text('Bereit – Board über die Weboberfläche einrichten.', (margin, height // 2), max_width=width - 2 * margin)
            elif mode == 'board':
                board_id = rotation[rotation_index].get('board_id') if rotation_index >= 0 else preferences.get('board_id')
                workspace = next((board for board in boards if board['boards'][0]['id'] == board_id), boards[0])
                board = workspace['boards'][0]
                text(board['name'], (round(190 * scale), margin), title_font, max_width=width - round(230 * scale))
                columns = visible(workspace)
                page_count = 1
                if columns:
                    shown_columns = min(len(columns), per_screen)
                    column_width = (width - margin * 2) // shown_columns
                    pages = paginate_columns(columns, shown_columns, rows)
                    page_count = len(pages)
                    if focus_highlight:
                        page = next((index for index, group in enumerate(pages)
                                     if any(ticket.get('id') in highlights for _, tickets, _ in group for ticket in tickets)), page)
                        focus_highlight = False
                    selected = pages[page % page_count]
                    for index, (column, tickets, total) in enumerate(selected):
                        x = margin + index * column_width
                        limit = column.get('wip_limit')
                        full = limit is not None and column.get('root_count', 0) >= limit
                        label = f"{column['name']} · {total}" + (f' · WIP {column["root_count"]}/{limit}' if full else '')
                        text(label, (x, round(100 * scale)), color='#e7bf82' if full else '#e5e7eb', max_width=column_width - margin)
                        for row, ticket in enumerate(tickets):
                            y = round(150 * scale) + row * card_height
                            highlighted = ticket.get('id') in highlights
                            background = '#28514b' if highlighted else '#17232d' if highlights else '#22313d'
                            foreground = '#a3adb4' if highlights and not highlighted else '#e5e7eb'
                            pygame.draw.rect(screen, background, (x, y, column_width - 12, card_height - 10), border_radius=8)
                            text(ticket['public_id'], (x + 10, y + 8), small, '#67e8c9' if foreground == '#e5e7eb' else foreground)
                            text(ticket['title'], (x + 10, y + round(38 * scale)), color=foreground, max_width=column_width - 32)
                            warning = due_warning(ticket, workspace, now)
                            if warning:
                                text(warning[0], (x + 10, y + round(68 * scale)), small, warning[1])
            elif mode == 'overview':
                overview = state.get('overview', [])
                text('Gesamtübersicht', (round(190 * scale), margin), title_font)
                page_count = max(1, (len(overview) + rows - 1) // rows)
                offset = (page % page_count) * rows
                for index, board in enumerate(overview[offset:offset + rows]):
                    y = round(125 * scale) + index * card_height
                    text(board['name'], (margin, y), max_width=width - 2 * margin)
                    summary = (f"{board['open']} offen · {board['done']} erledigt · "
                               f"{board['backlog']} Backlog · {board['blocked']} blockiert")
                    text(summary, (margin, y + round(32 * scale)), small, max_width=width - 2 * margin)
            else:
                text(f"Statistik · {preferences.get('statistics_days', 7)} Tage",
                     (round(190 * scale), margin), title_font, max_width=width - round(230 * scale))
                series = state.get('statistics', [])
                legend = [('Erstellt', '#67e8c9'), ('Abgeschlossen', '#7bbaff'), ('Backlog', '#e7bf82'), ('Blockiert', '#f09098')]
                bar_width = max(3, (width - 2 * margin) // 42)
                maximum = max((count for _, *counts in series for count in counts), default=1) or 1
                baseline = footer.top - round(65 * scale)
                for index, (label, color) in enumerate(legend):
                    text(label, (margin + index * (width - 2 * margin) // 4, round(96 * scale)), small, color)
                for index, (day, *counts) in enumerate(series):
                    x = margin + index * (width - 2 * margin) // 7
                    for column, (count, (_, color)) in enumerate(zip(counts, legend)):
                        size = round(count / maximum * max(0, baseline - round(135 * scale)))
                        if size:
                            pygame.draw.rect(screen, color, (x + column * bar_width, baseline - size, bar_width - 1, size))
                    text(day[5:], (x, baseline + margin), small)
                if not series:
                    text('Keine Daten im Zeitraum', (margin, height // 2))
                page_count = 1
            if qr_matrix and not overlay and mode == 'board' and not any(
                workspace.get('tickets') for workspace in boards
            ):
                module = max(2, min(6, height // (3 * len(qr_matrix))))
                extent = len(qr_matrix) * module
                x, y = width - margin - extent, footer.top - margin - extent
                if x >= margin and y >= 4 * margin:
                    pygame.draw.rect(screen, '#ffffff', (x, y, extent, extent))
                    for row, modules in enumerate(qr_matrix):
                        for column, dark in enumerate(modules):
                            if dark:
                                pygame.draw.rect(screen, '#000000', (
                                    x + column * module, y + row * module, module, module))
            if overlay:
                shade = pygame.Surface((width, footer.top), pygame.SRCALPHA)
                shade.fill((0, 0, 0, 150))
                screen.blit(shade, (0, 0))
                panel = pygame.Rect(margin, margin * 2, width - 2 * margin, footer.top - margin * 3)
                line_height = max(18, round(31 * scale))

                def draw_panel(bounds, entry, selected_page):
                    title, contents = display_content(boards, preferences, entry, mode)
                    pygame.draw.rect(screen, '#22313d', bounds, border_radius=8)
                    max_width = bounds.width - 2 * margin
                    text(title, (bounds.x + margin, bounds.y + margin), title_font, max_width=max_width)
                    visible_rows = max(1, (bounds.height - 4 * margin) // line_height)
                    width_chars = max(10, max_width // max(6, font.size('M')[0]))
                    pages, used = [[]], 0
                    for label, content in contents:
                        if entry['kind'] == 'ticket' and label in (None, 'Kommentar'):
                            rows = list(markdown_rows(content))
                            if label:
                                rows.insert(0, ('heading', label, None))
                        else:
                            rows = [('text', f'{label} · {content}' if label else content, None)]
                        for style, value, url in rows:
                            if style == 'image' and not images.enabled:
                                continue
                            lines = [value] if style == 'image' else textwrap.wrap(
                                value, width=width_chars, replace_whitespace=False,
                                drop_whitespace=style != 'code') or ['']
                            for line in lines:
                                size = min(visible_rows, 5) if style == 'image' else 1
                                if used + size > visible_rows:
                                    pages.append([])
                                    used = 0
                                pages[-1].append((used, size, style, line, url))
                                used += size
                    selected_page = min(selected_page, len(pages) - 1)
                    for row, size, style, line, url in pages[selected_page]:
                        x, y = bounds.x + margin, bounds.y + 3 * margin + row * line_height
                        image = images.get(url) if style == 'image' else None
                        if image:
                            pixels, resolution = image
                            source = pygame.image.frombytes(pixels, resolution, 'RGB')
                            factor = min(1, max_width / resolution[0], size * line_height / resolution[1])
                            image_size = (max(1, round(resolution[0] * factor)), max(1, round(resolution[1] * factor)))
                            screen.blit(pygame.transform.smoothscale(source, image_size), (x, y))
                            continue
                        color = '#9bc7e9' if style in ('table', 'heading') else '#e5e7eb'
                        if style in ('code', 'table'):
                            pygame.draw.rect(screen, '#17252f', (x, y, max_width, line_height - 1))
                        text(line, (x, y), color=color, max_width=max_width)
                    return selected_page

                if companion:
                    half = (panel.width - margin) // 2
                    left = pygame.Rect(panel.x, panel.y, half, panel.height)
                    draw_panel(left, companion, overlay_pages.get(tuple(companion.items()), 0))
                    panel.left += half + margin
                    panel.width = half
                overlay_page = draw_panel(panel, overlay, overlay_page)
            if previous is not None:
                transition = (previous, screen.copy(), time.monotonic(), duration)
                screen.blit(previous, (0, 0))
            pygame.display.flip()
            if state and 'instance' in state:
                companion_page = None
                if state.get('companion_ticket'):
                    identifier = state['companion_ticket']['ticket_id']
                    companion_page = [identifier, overlay_pages.get((('kind', 'ticket'), ('ticket_id', identifier)), 0)]
                current = state['revision'], mode, page, overlay, overlay_page, rotation_index, companion_page
                if current != last_saved:
                    write_runtime_view(state, mode, page, overlay, overlay_page, rotation_index, companion_page)
                    last_saved = current
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
