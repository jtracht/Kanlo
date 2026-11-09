"""Run inside a Debian container with --network none, no display, no privileges."""
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
import urllib.request

binary = os.environ.get('KANLO_TEST_BINARY', '/artifacts/kanlo-server-debug-x86_64-unknown-linux-gnu')
with tempfile.TemporaryDirectory() as directory:
    environment = {**os.environ, 'KANLO_DATA_DIR': directory, 'KANLO_BIND': '127.0.0.1:18080'}
    environment.pop('DISPLAY', None)
    environment.pop('WAYLAND_DISPLAY', None)
    for attempt in range(2):
        server = subprocess.Popen([binary], env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            for _ in range(100):
                try:
                    with urllib.request.urlopen('http://127.0.0.1:18080/api/v1/health', timeout=1) as response:
                        health = json.load(response)
                    break
                except OSError:
                    if server.poll() is not None:
                        raise RuntimeError(server.communicate())
                    time.sleep(0.1)
            else:
                raise RuntimeError('Server did not become ready')
            assert health['status'] == 'ok', health
            assert Path(directory).stat().st_mode & 0o077 == 0
            # An open event stream must not prevent a graceful service stop.
            stream = urllib.request.urlopen('http://127.0.0.1:18080/api/v1/events', timeout=3)
            server.send_signal(signal.SIGTERM)
            assert server.wait(timeout=8) == 0
            stream.close()
        finally:
            if server.poll() is None:
                server.kill()
            output, errors = server.communicate()
            print(output.decode(), end='')
            if errors:
                print(errors.decode(), end='')
    check = subprocess.run([binary, '--check'], env=environment, capture_output=True, text=True, check=True)
    assert check.stdout.strip() == 'ok'
print('PASS: Debian 13 headless, network-none, restart, private directory, SSE shutdown, integrity')
