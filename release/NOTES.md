# Kanlo 0.1.0-dev.1

Erster privater Entwicklungsrelease / First private development prerelease.

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

Entwicklungsversion, kein Stable 1.0. Native TV-Anzeige, Kiosk/Display-Setup,
OS-Images/Updater, vollständiges OpenAPI/MCP/ChatGPT und physische Pi-/SSD-
Abnahmen fehlen. ARM-Cross-Builds sind kein Hardwareleistungsnachweis.
Die vorhandene Lizenzdatei erteilt keine öffentliche Nutzungslizenz; dieser
Release bleibt im privaten Repository. Öffentliche Verteilung erfordert die
noch ausstehende Lizenz-/Markenfreigabe.

Development prerelease, not stable 1.0. Native TV/kiosk/display setup, OS images,
updater, complete OpenAPI/MCP/ChatGPT and physical Pi/SSD acceptance remain open.
Cross-compilation does not certify device performance. The current LICENSE does
not grant public distribution rights; this release stays private pending owner
licensing and trademark clearance.

## Installation

Siehe `INSTALL-DE.md` / See `INSTALL-EN.md` inside each platform archive.
Vor Nutzung `SHA256SUMS` und die Version in `BUILD.json` prüfen.
Verify `SHA256SUMS` and `BUILD.json` before running the binary.
