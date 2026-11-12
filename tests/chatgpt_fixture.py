"""Run a disposable local Kanlo instance for private ChatGPT acceptance."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
from urllib.request import Request, urlopen
from uuid import uuid4


def seed(origin):
    def api(path, data=None):
        request = Request(origin + '/api/v1/' + path,
                          data=None if data is None else json.dumps(data).encode(),
                          headers={'Origin': origin, 'Content-Type': 'application/json'})
        with urlopen(request, timeout=5) as response:
            return json.load(response)

    health = api('health')

    def mutate(command):
        return api('commands', {
            'op_id': str(uuid4()), 'client_id': 'chatgpt-acceptance',
            'server_instance_id': health['instance_id'], 'base_epoch': health['epoch'],
            'command': command,
        })['result']['entity_id']

    tickets = []
    for name, prefix in [('Abnahme Nord', 'NORD'), ('Abnahme Süd', 'SUED')]:
        board = mutate({'kind': 'create_board', 'name': name, 'prefix': prefix})
        ticket = mutate({'kind': 'create_ticket', 'board_id': board, 'title': 'WLAN-Test',
                         'description': '', 'importance': 50, 'column_id': None,
                         'parent_id': None, 'type_id': None})
        tickets.append(api(f'tickets/{ticket}')['public_id'])
    return tickets


def main(executable):
    with tempfile.TemporaryDirectory(prefix='kanlo-chatgpt-') as data:
        env = {**os.environ, 'KANLO_BIND': '127.0.0.1:0', 'KANLO_DATA_DIR': data}
        for key in ('KANLO_EXTERNAL_BIND', 'KANLO_BACKUP_DIR'):
            env.pop(key, None)
        with subprocess.Popen([str(executable.resolve())], env=env, stdout=subprocess.PIPE,
                              encoding='utf-8') as server:
            try:
                line = server.stdout.readline().strip()
                prefix = 'Kanlo LAN server listening on '
                if not line.startswith(prefix):
                    raise RuntimeError('Kanlo did not announce a local listening address')
                origin = line.removeprefix(prefix)
                tickets = seed(origin)
                print(f'KANLO_URL={origin}\nTesttickets: {", ".join(tickets)}', flush=True)
                print('Die Testdaten werden beim Beenden gelöscht. Strg+C beendet den Server.',
                      flush=True)
                server.wait()
            except KeyboardInterrupt:
                pass
            finally:
                if server.poll() is None:
                    server.terminate()
                    server.wait(timeout=10)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('server', type=Path, help='Path to the Kanlo server executable')
    main(parser.parse_args().server)
