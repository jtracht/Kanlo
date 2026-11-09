# Kanlo 0.2.0-dev.1

Private Geräte-Images / Private appliance development release.

## Geräte-Images / Appliance images

Vier flashbare `.img.xz`-Varianten: `rpi-armhf`, `rpi-arm64`, `generic-amd64`
und `generic-arm64`. Raspberry Pi OS Lite 13 bzw. Debian 13, vollständiger
Kanlo-Server mit eingebetteter Weboberfläche, automatischer Start, eigene
Dienstkonten, DHCP/mDNS und native SDL/Wayland-Boardanzeige ohne Chromium.
Ersteinrichtung lokal mit Ctrl+Alt+F2 oder `KANLO_CONFIG.json` auf der FAT-
Bootpartition. Keine gemeinsamen Kennwörter; SSH erst nach Public-Key-Einrichtung.
Automatische Dateisystemerweiterung, begrenzte flüchtige Logs und ZRAM.

Each image must pass two QEMU kernel/systemd boots, a real ticket write, reboot
persistence, server restart, private storage, provisioning, root expansion and
SQLite integrity checks. `.image.json` records the pinned upstream base,
installed OS packages, source commit, checksum and emulator evidence.
Read `IMAGES-DE.md` / `IMAGES-EN.md` for compatible hardware and flashing/setup.

## Enthalten / Included

- Kanban-Domainkern mit SQLite, stabilen Kennungen, Typen, Labels, eigenen
  Textfeldern, Untertickets, Abhängigkeiten und WIP-Regeln.
- Bearbeitbare DE-/EN-Weboberfläche mit Themes, HTTPS-PWA, dauerhafter Offline-
  Queue, expliziter Konfliktauflösung und Mehrtab-Abgleich.
- Undo, Papierkorb/Archiv, Wiederholung, JSON/CSV, konsistente Datenbackups und
  selektive/vollständige Wiederherstellung mit neuer Datenepoche.
- Getrennte LAN-/geschützte Proxy-Zugänge, Sessions/CSRF und widerrufbare API-Keys.
- Debian-13-Serverpakete für AMD64, ARM64 und ARM32/armhf; Prüfsummen und
  Quellstandzuordnung. Alle Webassets sind eingebettet; kein Node/Docker zur Laufzeit.

## Grenzen / Known limits

Entwicklungsversion, kein Stable 1.0. Die native Anzeige ist eine erste passive
Boardansicht; vollständige TV-Detailoverlays/Markdown/Statistik/Steuerung, Updater,
vollständiges OpenAPI/MCP/ChatGPT und physische Pi-/SSD-Abnahmen bleiben offen.
QEMU-Boots sind kein realer Hardware-/HDMI-/WLAN-Leistungsnachweis.
Die vorhandene Lizenzdatei erteilt keine öffentliche Nutzungslizenz; dieser
Release bleibt im privaten Repository. Öffentliche Verteilung erfordert die
noch ausstehende Lizenz-/Markenfreigabe.

Development prerelease, not stable 1.0. The native display provides initial
read-only boards; full TV overlays/Markdown/statistics/control, the updater,
complete OpenAPI/MCP/ChatGPT and physical Pi/SSD acceptance remain open.
Emulated boots do not certify physical device performance. The current LICENSE does
not grant public distribution rights; this release stays private pending owner
licensing and trademark clearance.

## Installation

Siehe `INSTALL-DE.md` / See `INSTALL-EN.md` inside each platform archive.
Vor Nutzung `SHA256SUMS` und die Version in `BUILD.json` prüfen.
Verify `SHA256SUMS` and `BUILD.json` before running the binary.
