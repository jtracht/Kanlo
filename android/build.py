"""Build and verify an installable Kanlo TV preview APK."""
import argparse
import os
from pathlib import Path
import subprocess
import tempfile
from zipfile import ZIP_DEFLATED, ZipFile


def build(args):
    source = Path(__file__).resolve().parent
    executable = '.exe' if os.name == 'nt' else ''
    tools = args.build_tools.resolve()
    platform = args.platform.resolve()
    jdk = args.jdk.resolve() / 'bin'

    def run(command, *values):
        subprocess.run([command, *values], check=True)

    def java(jar, *values):
        run(jdk / ('java' + executable), '-jar', tools / 'lib' / jar, *values)

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.keystore.parent.mkdir(parents=True, exist_ok=True)
    if not args.keystore.exists():
        run(jdk / ('keytool' + executable), '-genkeypair', '-keystore', args.keystore,
            '-storepass', 'android', '-keypass', 'android', '-alias', 'androiddebugkey',
            '-keyalg', 'RSA', '-keysize', '2048', '-validity', '10000',
            '-dname', 'CN=Kanlo TV Local Preview', '-noprompt')

    with tempfile.TemporaryDirectory() as directory:
        work = Path(directory)
        classes = work / 'classes'
        classes.mkdir()
        unsigned = work / 'unsigned.apk'
        aligned = work / 'aligned.apk'
        run(jdk / ('javac' + executable), '--release', '8', '-cp', platform, '-d', classes,
            source / 'MainActivity.java')
        run(jdk / ('java' + executable), '-cp', tools / 'lib' / 'd8.jar',
            'com.android.tools.r8.D8', '--lib', platform, '--min-api', '26', '--output', work,
            *sorted(classes.rglob('*.class')))
        run(tools / ('aapt2' + executable), 'link', '-I', platform, '--manifest',
            source / 'AndroidManifest.xml', '-o', unsigned)
        with ZipFile(unsigned, 'a', compression=ZIP_DEFLATED) as apk:
            apk.write(work / 'classes.dex', 'classes.dex')
        run(tools / ('zipalign' + executable), '-f', '4', unsigned, aligned)
        java('apksigner.jar', 'sign', '--ks', args.keystore, '--ks-key-alias', 'androiddebugkey',
             '--ks-pass', 'pass:android', '--key-pass', 'pass:android', '--out', args.output, aligned)
        java('apksigner.jar', 'verify', '--verbose', args.output)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    for name in ('build-tools', 'platform', 'jdk', 'keystore', 'output'):
        parser.add_argument('--' + name, required=True, type=Path)
    build(parser.parse_args())
