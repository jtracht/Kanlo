# Changelog

## 0.2.0-dev.10

- Kanlo groups settings by category and exposes names in pages of five.
- ChatGPT and Claude Code can request explicit setting values without receiving credentials.
- Stable release approval records can be committed after the reviewed candidate.

## 0.2.0-dev.9

- The appliance opens Wi-Fi setup before the board on first interactive boot.
- The local settings menu provides Wi-Fi editing, Bluetooth controls and display diagnostics.

## 0.2.0-dev.8

- Native TV panels render Markdown tables, code blocks, comments and opt-in external images.
- The TV can show two independent tickets with individual durations and restores both scroll positions after a restart.

## 0.2.0-dev.7

- Local setup offers a QR code and a temporary WPA2 hotspot.
- The explicit Linux updater validates a package and backs up the executable and database before migration.
- Settings show the installed version and manual updater availability.

## 0.2.0-dev.6 (lokale Entwicklung)

- TV-Anzeige mit Ansichtswechsel, Rotation, Ticket-, Backlog- und
  Unterticket-Einblendungen, Hervorhebungen und Anzeige-Warnhinweisen.
- Boardübergreifende Statusübersicht für Web und TV sowie native Statistik.
- MCP-Steuerung der Anzeige für Claude Code und ChatGPT. Einrichtungsdialog
  mit Schlüsseltest und Leseschema für GPT Actions.
- Optionaler Kiosk-Modus und Deinstallation im Standalone-Serverinstaller.

## 0.2.0-dev.5

- Native Anzeige und Konsolen-Fallback beziehen Änderungen aus dem Kanlo-
  Ereignisstrom und gleichen Boards nach einem Serverneustart erneut ab.
- Formatprüfungen für kleine, hochkantige, breite und Ultrawide-Anzeigen sowie
  unabhängige Neustarttests für Server und Anzeigedienst.
- Serverpakete enthalten ein Installationsskript mit Architektur- und
  Integritätsprüfung. Die deutsch- und englischsprachigen Anleitungen wurden
  an den Gerätebetrieb angepasst.
- Entwicklungsstand nach Originalaufgaben **60 % (42 von 70)**.

## 0.2.0-dev.4

- Die Geräteanzeige lässt den Wechsel zur lokalen Einrichtung zu und zeigt
  bei einem Fehler im Grafikstart ein einfaches Board auf der Konsole.
- DRM-Hotplug kann die Anzeige starten, ohne die laufende Sitzung neu zu
  starten. Composite-Ausgänge mit einem verfügbaren Modus werden erkannt.
- Die lokale Einrichtung bietet eine WLAN-Auswahl und eine Anzeige-Diagnose.
- Belegter Entwicklungsstand nach Originalaufgaben **50 % (35 von 70)**.

## 0.2.0-dev.3

- Die API liefert Ticketkommentare seitenweise. Beide Plugins können Kommentare
  lesen und mit freigegebenem Schreibzugriff Tickets in Zielspalten oder ins
  Backlog verschieben.

## 0.2.0-dev.2

- Die API bietet eine paginierte, boardübergreifende Ticketsuche mit Text-,
  Spalten- und Statusfiltern auf Basis der Kanlo-Domänensuche.
- Claude Code und ChatGPT nutzen dieselbe Suche und können mit freigegebenem
  Schreibzugriff Tickets bearbeiten und kommentieren.

## 0.2.0-dev.1

- Eine bereits vor dem ersten Ereignisstrom widerrufene externe Sitzung fordert
  auch ohne ausstehende Offline-Änderungen zuverlässig zur Anmeldung auf.
- Vier dedizierte, automatisch gebaute Geräte-Images: Raspberry Pi ARM32/ARM64
  und allgemeine AMD64-/ARM64-UEFI-Geräte, mit gepinnten offiziellen OS-Basen.
- Kanlo-Autostart, separate native SDL/Wayland-Boardanzeige, Headless-/Hotplug-
  Betrieb, lokaler Einrichtungsdialog und gerätespezifische Bootkonfiguration.
- DHCP/mDNS, exakte LAN-Hosts, keine gemeinsamen Passwörter, SSH erst mit eigenem
  Public Key, automatische Speichererweiterung, begrenzte Logs und ZRAM.
- Image-Veröffentlichung verlangt Erststart/Reboot, persistierte Testtickets,
  Dienste-Neustart, private Dateirechte und SQLite-Integrität auf einer isolierten
  Testkopie; OS-Paketlisten, Basis-/Quellstand und Prüfsummen liegen bei.
- Große Release-Anhänge werden mit begrenztem Speicherverbrauch übertragen und
  erneut heruntergeladen/geprüft. Alte Releases und Tags bleiben unverändert.

## 0.1.0-dev.1 — first private development prerelease

- Deliver the transactional domain, editable DE/EN web app, durable offline
  synchronization, conflict handling and protected external access.
- Add children/dependencies/types/labels/custom fields, undo, trash/archive,
  recurrence and data-only import/export/backup/restore.
- Package Debian 13 server binaries for AMD64, ARM64 and armhf with source,
  build metadata, SHA256 checksums, third-party notices and DE/EN instructions.
- Gate automatic tag releases on all CI jobs and real packaged-binary startup.
- Keep native TV, kiosk/images/updater, physical device acceptance, complete
  OpenAPI/MCP and final licensing outside this development release.

### Initial groundwork

- Preserve the three original requirement documents and review scope differences.
- Establish Rust domain/storage/API workspace and locked dependencies.
- Add transactional SQLite schema, stable identity/epoch and initial board.
- Add boards, columns, tickets and revision-aware moves with strict WIP validation.
- Add provisional local browser client and loopback/Origin/Host development guards.
- Add concurrency, persistence, migration and HTTP regression tests.
- Prepare Debian 13 architecture CI and M0–M10 development plan.
