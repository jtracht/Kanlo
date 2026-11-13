# Kanlo 1.0.0

Erste stabile Version von Kanlo.

Web- und TV-Board haben ein neues Layout. Auf der lokalen Anzeige bewegt GTK
vorbereitete Bilder im Takt des Compositors. Spaltenseiten und gerenderter
Text werden zwischengespeichert, um die CPU beim Ansichtswechsel zu entlasten.

Kanlo liefert für ChatGPT und Claude Code zuerst Einstellungskategorien,
dann Namen in Fünferseiten. Werte werden ausdrücklich abgerufen. Bei
Board-Regeln ist dazu das konkrete Board erforderlich.

Die versionierte API veröffentlicht ihren OpenAPI-3.1-Vertrag unter
`/api/v1/openapi.json`. Der Integrationsassistent erzeugt daraus das GPT-Actions-
Leseschema. Die native TV-Anzeige speichert ihre zuletzt sichtbare Position für
den Wiederanlauf. Die lokale Speicherabnahme prüft Onlinebackup, SSD-Wechsel,
Rückwechsel und Wiederherstellung.

Die Weboberfläche und beide MCP-Plugins steuern die native TV-Anzeige.
Sie bietet Board, Gesamtübersicht, Statistik, Rotation, Hervorhebungen
und Einblendungen für Tickets, Backlog und Untertickets.
Die Weboberfläche enthält einen Einrichtungsdialog für MCP und GPT Actions.
Ticketfenster nutzen natives Markdown und eigene Zeitlimits. Bei ausreichender
Auflösung stehen zwei Fenster nebeneinander. Externe Bilder sind lokal
freischaltbar.

Die native Anzeige verarbeitet Live-Änderungen und gleicht ihre Boards
nach einem Serverneustart erneut ab. Die Serverpakete enthalten
`install.sh` mit Architektur- und Prüfsummenprüfung.

Der manuelle Updater sichert vor der Migration Datenbank und Binärdatei.
Die Weboberfläche zeigt die installierte Version auf Anfrage.

Die Geräteanzeige erlaubt den Wechsel zwischen Board und lokaler Einrichtung.
Bei einem Grafikfehler steht eine einfache Boardansicht auf der Konsole zur
Verfügung. Beim ersten Start öffnet sich die WLAN-Ersteinrichtung vor dem
Board. Die Tastaturbelegung lässt sich davor auswählen. Kanlo aktiviert WLAN
und scannt vor der Netzwerkliste. Nach „Fertig“ wechselt Kanlo zur Board-Konsole.
Die Weboberfläche im Geräte-Image ist über HTTP-Port 80 erreichbar. Systemmeldungen
bleiben von den Menüs getrennt. Die Geräteeinstellungen bieten WLAN,
Bluetooth und die Anzeige-Diagnose. Composite hängt vom Pi-Modell und der Firmware-
Konfiguration ab.

Die Kanlo-API liefert Ticketkommentare seitenweise. Die separat erhältlichen
Kanlo-Plugins 0.6.1 für Claude Code und ChatGPT können Kommentare lesen und
Tickets in Zielspalten oder ins Backlog verschieben. Sie bieten außerdem
eine Vorschau mit separater Bestätigung für Rückgängig-Aktionen und ändern
einzelne Einstellungen mit Prüfung des zuvor gelesenen Werts.

## Geräte-Images / Appliance images

Vier flashbare `.img.xz`-Varianten: `rpi-armhf`, `rpi-arm64`, `generic-amd64`
und `generic-arm64`. Raspberry Pi OS Lite 13 bzw. Debian 13, vollständiger
Kanlo-Server mit eingebetteter Weboberfläche, automatischer Start, eigene
Dienstkonten, DHCP/mDNS und eine native Boardanzeige mit Weston und GTK/Wayland.
WLAN-Ersteinrichtung auf dem Bildschirm oder vorab per `KANLO_CONFIG.json`
auf der FAT-Bootpartition. Keine gemeinsamen Kennwörter; SSH erst nach Public-Key-Einrichtung.
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

Der Anwender bestätigte SD-Erststart, WLAN-Liste, lokales Board und Neustart
auf dem Pi 3 B+. Die Dauerlastmessungen auf den Zielgeräten stehen aus.
Die manuellen Workspace-Tests übernimmt der Anwender. Kanlo steht unter der
PolyForm Noncommercial License 1.0.0.

The owner confirmed SD boot, Wi-Fi setup, local board and reboot on a Pi 3 B+.
Extended benchmarks on target devices remain pending. The owner will run the
manual ChatGPT workspace tests. Kanlo is licensed under the PolyForm
Noncommercial License 1.0.0.

## Installation

Siehe `INSTALL-DE.md` / See `INSTALL-EN.md` inside each platform archive.
Vor Nutzung `SHA256SUMS` und die Version in `BUILD.json` prüfen.
Verify `SHA256SUMS` and `BUILD.json` before running the binary.
