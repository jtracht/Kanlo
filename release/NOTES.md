# Kanlo 0.2.0-dev.6

Lokaler Entwicklungsstand **85,7 % (60 von 70 Originalaufgaben)**.

Die versionierte API veröffentlicht ihren OpenAPI-3.1-Vertrag unter
`/api/v1/openapi.json`. Der Integrationsassistent erzeugt daraus das GPT-Actions-
Leseschema. Die native TV-Anzeige speichert ihre zuletzt sichtbare Position für
den Wiederanlauf. Die lokale Speicherabnahme prüft Onlinebackup, SSD-Wechsel,
Rückwechsel und Wiederherstellung.

Die Weboberfläche und beide MCP-Plugins steuern die native TV-Anzeige.
Sie bietet Board, Gesamtübersicht, Statistik, Rotation, Hervorhebungen
und Einblendungen für Tickets, Backlog und Untertickets.
Die Weboberfläche enthält einen Einrichtungsdialog für MCP und GPT Actions.

Die native Anzeige verarbeitet Live-Änderungen und gleicht ihre Boards
nach einem Serverneustart erneut ab. Die Serverpakete enthalten
`install.sh` mit Architektur- und Prüfsummenprüfung.

Die Geräteanzeige erlaubt den Wechsel zwischen Board und lokaler Einrichtung.
Bei einem Grafikfehler steht eine einfache Boardansicht auf der Konsole zur
Verfügung. Über Strg+Alt+F2 kann der Gerätebesitzer WLAN auswählen und die
Anzeige diagnostizieren. Composite hängt vom Pi-Modell und der Firmware-
Konfiguration ab. Die physische Anschlussprüfung steht noch aus.

Die Kanlo-API liefert Ticketkommentare seitenweise. Die separat erhältlichen
Kanlo-Plugins 0.3.0 für Claude Code und ChatGPT können Kommentare lesen und
Tickets in Zielspalten oder ins Backlog verschieben.

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

Entwicklungsversion. Vollständige native Markdown- und Bilddarstellung,
automatisierte Update-Wiederherstellung, die ChatGPT-Abnahme im Workspace
sowie physische Raspberry-Pi-Tests stehen aus. Die QEMU-Prüfung erfasst den
emulierten Systemstart. Tests für HDMI, Composite und WLAN am Gerät stehen aus.
Die öffentliche Verteilung erfordert die ausstehende Lizenz- und Markenfreigabe.

Development prerelease. Full native Markdown and image rendering, automated
update recovery, ChatGPT workspace acceptance and physical Raspberry Pi testing
are pending. QEMU verifies the emulated boot. On-device HDMI, composite and
Wi-Fi testing remain pending. Public distribution requires licensing and
trademark clearance.

## Installation

Siehe `INSTALL-DE.md` / See `INSTALL-EN.md` inside each platform archive.
Vor Nutzung `SHA256SUMS` und die Version in `BUILD.json` prüfen.
Verify `SHA256SUMS` and `BUILD.json` before running the binary.
