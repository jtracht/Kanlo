"""Run an isolated native display, interrupt it, then exercise the restored view."""
import argparse
import json
import os
from pathlib import Path
import resource
import signal
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'packaging/images'))
import display


def child(path, seconds, resume):
    import pygame

    display.RUNTIME_VIEW = path
    ticket = {'id': 'ticket', 'board_id': 'board', 'public_id': 'KAN-1',
              'column_id': 'todo', 'title': 'Soak test ticket', 'done': False,
              'description': '\n'.join(f'Line {n}' for n in range(180)),
              'details': {'archived': False, 'trashed': False, 'rank': 1}}
    workspace = {'boards': [{'id': 'board', 'name': 'Soak test board'}],
                 'columns': [{'id': 'todo', 'name': 'Open', 'position': 0}],
                 'tickets': [ticket, {**ticket, 'id': 'companion', 'public_id': 'KAN-2'}]}
    overlay = {'kind': 'ticket', 'ticket_id': 'ticket'}

    def feed(updates, stop, display_updates):
        display.offer(updates, ([workspace], True))
        state = {'preferences': {'view': 'board', 'rotation': []},
                 'revision': 8 if resume else 7, 'instance': ['fixture', 'epoch'],
                 'scroll_offset': 5, 'animation': 'off', 'overlays': [overlay],
                 'companion_ticket': {'ticket_id': 'companion', 'expires_at': None}}
        if not resume:
            display.offer(display_updates, {**state, 'revision': 5, 'scroll_offset': 0,
                          'overlays': [{'kind': 'ticket', 'ticket_id': 'companion'}],
                          'companion_ticket': None})
            stop.wait(0.6)
            display.offer(display_updates, {**state, 'revision': 6, 'scroll_offset': 2,
                          'overlays': [{'kind': 'ticket', 'ticket_id': 'companion'}],
                          'companion_ticket': None})
            stop.wait(0.6)
            display.offer(display_updates, {**state, 'scroll_offset': 2})
            stop.wait(1)
            display.offer(display_updates, {**state, 'revision': 8})
            stop.wait(seconds)
        else:
            display.offer(display_updates, state)
            stop.wait(2)
            saved = json.loads(path.read_text())
            (path.parent / 'restored.json').write_text(json.dumps(saved))
            start = time.monotonic()
            tick = 0
            while time.monotonic() - start < seconds and not stop.wait(1):
                animation = ('off', 'subtle', 'fluid', 'dynamic')[tick % 4]
                overlays = [overlay] if tick % 2 else []
                display.offer(display_updates, {**state, 'revision': 9 + tick,
                              'overlays': overlays, 'animation': animation})
                tick += 1
        pygame.event.post(pygame.event.Event(pygame.QUIT))

    display.reader = feed
    display.main()
    peak = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024
    assert peak < 230, f'Renderer peak was {peak:.1f} MiB'
    print(f'PASS: renderer peak {peak:.1f} MiB', flush=True)


def wait_json(path, condition, seconds):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        try:
            value = json.loads(path.read_text())
            if condition(value):
                return value
        except (OSError, ValueError):
            pass
        time.sleep(0.1)
    raise AssertionError(f'Missing expected display state in {path.name}')


def parent(seconds):
    with tempfile.TemporaryDirectory() as directory:
        runtime = Path(directory) / 'wayland'
        runtime.mkdir(mode=0o700)
        path = Path(directory) / 'view.json'
        env = {**os.environ, 'XDG_RUNTIME_DIR': str(runtime), 'WLR_BACKENDS': 'headless',
               'WLR_HEADLESS_OUTPUTS': '1', 'WLR_RENDERER': 'pixman',
               'SDL_VIDEODRIVER': 'wayland', 'SDL_VIDEO_WAYLAND_ALLOW_LIBDECOR': '0',
               'LP_NUM_THREADS': '2'}
        command = ['cage', '--', sys.executable, __file__, '--state', str(path)]
        first = subprocess.Popen(command + ['--seconds', '60'], env=env, start_new_session=True)
        try:
            wait_json(path, lambda value: value['overlay_page'] == 3
                      and value.get('companion_page') == ['companion', 2], 15)
        finally:
            if first.poll() is None:
                os.killpg(first.pid, signal.SIGKILL)
            first.wait()
        second = subprocess.Popen(command + ['--resume', '--seconds', str(seconds)],
                                  env=env, start_new_session=True)
        try:
            wait_json(Path(directory) / 'restored.json', lambda value: value['overlay_page'] == 3
                      and value.get('companion_page') == ['companion', 2], 15)
            assert second.wait(timeout=seconds + 20) == 0
        finally:
            if second.poll() is None:
                os.killpg(second.pid, signal.SIGKILL)
                second.wait()
        print(f'PASS: previous overlay position restored after process kill, {seconds}s native soak')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--seconds', type=int, default=600)
    parser.add_argument('--state', type=Path)
    parser.add_argument('--resume', action='store_true')
    args = parser.parse_args()
    if args.state:
        child(args.state, args.seconds, args.resume)
    else:
        parent(args.seconds)
