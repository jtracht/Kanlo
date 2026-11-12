"""Two real kernel/systemd boots on a disposable raw copy, TCG, no KVM needed."""
import gzip
import json
from pathlib import Path
import shutil
import subprocess
import time


def verify(image, profile, directory):
    from build import ROOT, checksum, mounted, run
    test = directory / 'boot-test.img'
    run('cp', '--sparse=always', image, test)
    run('truncate', '-s', '16G', test)
    with mounted(test, directory) as (root, root_part, parts):
        shutil.copy2(ROOT / 'packaging/images/boot_guest.py', root / 'usr/lib/kanlo/boot_guest.py')
        if profile == 'generic-amd64':
            shutil.copy2(ROOT / 'packaging/images/boot_wifi.py', root / 'usr/lib/kanlo/boot_wifi.py')
        unit = root / 'etc/systemd/system/kanlo-image-test.service'
        unit.write_text('[Unit]\nAfter=kanlo-server.service kanlo-setup.service\nWants=kanlo-server.service kanlo-setup.service\n[Service]\nType=oneshot\nExecStart=/usr/bin/python3 /usr/lib/kanlo/boot_guest.py\nStandardOutput=journal+console\nStandardError=journal+console\nTimeoutStartSec=240\n[Install]\nWantedBy=multi-user.target\n')
        wants = root / 'etc/systemd/system/multi-user.target.wants'
        wants.mkdir(exist_ok=True)
        (wants / unit.name).symlink_to('../' + unit.name)
        family, arch = profile.split('-')
        provision = root / ('boot/firmware' if family == 'rpi' else 'boot/efi')
        (provision / 'KANLO_CONFIG.json').write_text(json.dumps({'hostname': 'kanlo-image-test', 'display': False, 'wifi': {'ssid': 'Kanlo fixture offline', 'password': 'fixture-only-no-real-secret', 'country': 'DE'}}))
        root_id = root_part['partuuid']
        firmware = directory / 'firmware'; firmware.mkdir(exist_ok=True)
        if family == 'rpi':
            boot = root / 'boot/firmware'
            # Pi 3 is a supported target for both userspaces. The vendor's
            # 64-bit kernel also runs ARMHF userspace (as on 32-bit Pi OS/Pi 4).
            names = ['kernel8.img', 'bcm2710-rpi-3-b.dtb', 'initramfs8']
            for name in names:
                shutil.copy2(boot / name, firmware / name)
            # Direct -kernel boot bypasses the Pi firmware's gzip decompression.
            # The minimal pinned emulator has no optional gzip loader. Preserve
            # the vendor kernel and decompress only its isolated test copy.
            kernel = firmware / names[0]
            with kernel.open('rb') as stream:
                compressed = stream.read(2) == b'\x1f\x8b'
            if compressed:
                unpacked = firmware / 'kernel8-uncompressed.img'
                with gzip.open(kernel, 'rb') as source, unpacked.open('wb') as target:
                    shutil.copyfileobj(source, target)
                unpacked.replace(kernel)
            machine = 'raspi3b'
            executable = 'qemu-system-aarch64'
            command = [executable, '-M', machine, '-m', '1024', '-kernel', str(firmware / names[0]), '-dtb', str(firmware / names[1]), '-initrd', str(firmware / names[2]), '-append', f'root=PARTUUID={root_id} rootfstype=ext4 rootwait earlycon=pl011,mmio32,0x3f201000 console=ttyAMA1,115200 systemd.show_status=1', '-drive', f'file={test},if=none,id=disk,format=raw', '-device', 'usb-hub', '-device', 'usb-storage,drive=disk']
            # QEMU's USB model uses ordinary interrupts, not the Pi firmware's
            # optional FIQ acceleration. This command line is test-only.
            command[command.index('-append') + 1] += ' dwc_otg.fiq_enable=0 dwc_otg.fiq_fsm_enable=0'
            command.extend(['-netdev', 'user,id=kanlo', '-device', 'usb-net,netdev=kanlo'])
        elif arch == 'amd64':
            code = Path('/usr/share/OVMF/OVMF_CODE_4M.fd')
            variables = firmware / 'vars.fd'
            shutil.copy2('/usr/share/OVMF/OVMF_VARS_4M.fd', variables)
            command = ['qemu-system-x86_64', '-M', 'q35', '-accel', 'tcg', '-m', '1024', '-smp', '2', '-drive', f'if=pflash,format=raw,readonly=on,file={code}', '-drive', f'if=pflash,format=raw,file={variables}', '-drive', f'file={test},if=virtio,format=raw', '-nic', 'user,model=virtio-net-pci']
        else:
            code = Path('/usr/share/AAVMF/AAVMF_CODE.fd')
            variables = firmware / 'vars.fd'
            shutil.copy2('/usr/share/AAVMF/AAVMF_VARS.fd', variables)
            command = ['qemu-system-aarch64', '-M', 'virt', '-cpu', 'cortex-a72', '-accel', 'tcg', '-m', '1024', '-smp', '2', '-drive', f'if=pflash,format=raw,readonly=on,file={code}', '-drive', f'if=pflash,format=raw,file={variables}', '-drive', f'file={test},if=virtio,format=raw', '-nic', 'user,model=virtio-net-pci']
    results = []
    for phase in ['firstboot', 'reboot']:
        log_path = directory / f'{phase}.log'
        with log_path.open('w') as log:
            process = subprocess.Popen([*command, '-L', '/usr/share/qemu', '-display', 'none', '-serial', 'stdio', '-monitor', 'none', '-no-reboot'], stdout=log, stderr=subprocess.STDOUT, text=True)
            try:
                process.wait(timeout=600)
            except subprocess.TimeoutExpired:
                process.kill(); process.wait()
                print(log_path.read_text(errors='replace')[-12000:], flush=True)
                raise RuntimeError(f'{profile} {phase} boot timed out') from None
        log = log_path.read_text(errors='replace')
        marker = 'KANLO_IMAGE_BOOT_OK '
        reports = [line.split(marker, 1)[1] for line in log.splitlines() if marker in line]
        if process.returncode != 0 or not reports or 'KANLO_IMAGE_BOOT_FAILED ' in log:
            print(log[-15000:], flush=True)
            raise RuntimeError(f'{profile} {phase} failed (QEMU exit {process.returncode})')
        report = json.loads(reports[-1])
        if report['phase'] != phase:
            raise ValueError('Reboot did not retain the first boot identity/database')
        if report['userspace_arch'] != arch:
            raise ValueError('Booted userspace architecture does not match the profile')
        results.append(report)
        print('PASS: QEMU', profile, phase, flush=True)
    test.unlink()
    emulator = run(command[0], '--version', capture_output=True).stdout.splitlines()[0]
    source = Path('/usr/local/share/kanlo-emulator-commit')
    emulator_commit = source.read_text().strip() if arch != 'amd64' and source.exists() else None
    return {'status': 'passed', 'emulator': emulator + ' (TCG)', 'emulator_source_commit': emulator_commit, 'emulator_binary_sha256': checksum(Path(shutil.which(command[0]))), 'machine': command[command.index('-M') + 1], 'kernel_arch': 'arm64' if family == 'rpi' else arch, 'userspace_arch': arch, 'disk_bus': 'usb-storage' if family == 'rpi' else 'virtio', 'boots': results, 'physical_device_certified': False}
