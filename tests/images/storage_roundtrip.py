"""Test online backup and a data-directory round trip with a mounted SSD destination."""
import contextlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
import uuid

ORIGIN = 'http://127.0.0.1:18090'


def api(path, data=None, headers=None):
    request = urllib.request.Request(
        f'{ORIGIN}/api/v1/{path}',
        data=None if data is None else json.dumps(data).encode(),
        headers={'Origin': ORIGIN, 'Content-Type': 'application/json', **(headers or {})},
    )
    with urllib.request.urlopen(request, timeout=5) as response:
        return json.load(response)


def environment(directory):
    return {**os.environ, 'KANLO_DATA_DIR': str(directory), 'KANLO_BIND': '127.0.0.1:18090'}


@contextlib.contextmanager
def server(binary, directory):
    process = subprocess.Popen([binary], env=environment(directory))
    try:
        for _ in range(60):
            if process.poll() is not None:
                raise RuntimeError(f'Server exited with {process.returncode}')
            try:
                api('health')
                break
            except OSError:
                time.sleep(0.1)
        else:
            raise RuntimeError('Server did not become ready')
        yield
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=15)


def create_ticket(board, title):
    health = api('health')
    return api(f'boards/{board}/tickets', {'title': title}, {
        'Idempotency-Key': str(uuid.uuid4()),
        'X-Kanlo-Client': 'storage-acceptance',
        'X-Kanlo-Instance': health['instance_id'],
        'X-Kanlo-Epoch': health['epoch'],
    })


def migrate(binary, source, target):
    subprocess.run([binary, '--migrate-data', target], env=environment(source), check=True)
    result = subprocess.check_output([binary, '--check'], env=environment(target), text=True)
    assert result.strip() == 'ok'


def main(binary, mount):
    assert mount.is_dir()
    with tempfile.TemporaryDirectory(dir='/dev/shm') as memory, tempfile.TemporaryDirectory(dir=mount) as ssd:
        source, rollback = Path(memory) / 'source', Path(memory) / 'rollback'
        target = Path(ssd) / 'kanlo'
        assert Path(memory).stat().st_dev != Path(ssd).stat().st_dev
        with server(binary, source):
            board = api('boards')[0]['id']
            ticket = create_ticket(board, 'Before SSD migration')
            preview = api('imports/csv/preview', {
                'board_id': board, 'csv': 'title,importance\nImported over HTTP,42\n',
            })
            assert 'Imported over HTTP' not in json.dumps(api(f'boards/{board}/tickets'))
            api('recovery/confirm', {
                'token': preview['token'], 'op_id': str(uuid.uuid4()),
                'client_id': 'storage-acceptance',
            })
            imported = next(row for row in api(f'boards/{board}/tickets')
                            if row['title'] == 'Imported over HTTP')
            assert imported['importance'] == 42
            key = api('security/keys', {'name': 'Acceptance fixture'})
            backup = api('backups', {})
            assert ticket['title'] in json.dumps(backup)
            assert key['token'] not in json.dumps(backup)
            assert key['id'] not in json.dumps(backup)
            migrate(binary, source, target)
            try:
                create_ticket(board, 'Blocked old writer')
            except urllib.error.HTTPError as error:
                assert error.code == 409
            else:
                raise AssertionError('Old data directory still accepts writes')
        with server(binary, target):
            assert api(f"tickets/{ticket['id']}")['title'] == ticket['title']
            new_ticket = create_ticket(board, 'Saved while on SSD')
        migrate(binary, target, rollback)
        with server(binary, rollback):
            assert api(f"tickets/{ticket['id']}")['title'] == ticket['title']
            assert api(f"tickets/{new_ticket['id']}")['title'] == new_ticket['title']
            preview = api('backups/restore/preview', {'backup': backup, 'boards': None})
            old_epoch = api('health')['epoch']
            api('recovery/confirm', {
                'token': preview['token'], 'op_id': str(uuid.uuid4()),
                'client_id': 'storage-acceptance',
            })
            assert api('health')['epoch'] != old_epoch
            assert api(f"tickets/{ticket['id']}")['title'] == ticket['title']
            assert api('security/keys') == []
            assert api(f"tickets/{imported['id']}")['importance'] == 42
        print('PASS: online backup and restore, SSD migration and lossless return')


if __name__ == '__main__':
    main(sys.argv[1], Path(sys.argv[2]))
