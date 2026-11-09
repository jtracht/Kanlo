"""Verify/extract the actual packaged binary and exercise its runtime."""
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile

root = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(root / 'scripts'))
from release_package import inspect

archive = Path(sys.argv[1])
if archive.is_dir():
    candidates = list(archive.glob('kanlo-*-linux-amd64.tar.gz'))
    if len(candidates) != 1:
        raise ValueError('Exactly one AMD64 platform package is required')
    archive = candidates[0]
build = inspect(archive)
with tempfile.TemporaryDirectory() as directory:
    with tarfile.open(archive) as tar:
        tar.extractall(directory, filter='data')
    binary = next(Path(directory).glob('*/bin/kanlo-server'))
    environment = {**os.environ, 'KANLO_TEST_BINARY': str(binary), 'KANLO_EXPECT_VERSION': build['version']}
    subprocess.run([sys.executable, str(root / 'packaging/tests/headless.py')], env=environment, check=True)
print('PASS: packaged AMD64 binary, embedded version, restart and integrity')
