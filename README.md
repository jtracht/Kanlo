# Kanlo

Lokales, TV-zentriertes Kanban für ein Debian-/Raspberry-Pi-Gerät.
**Entwicklungsstand 0.1.0-dev.1 — keine fertige Version 1.0.**

Die drei Originalunterlagen liegen unverändert unter `docs/baseline/`.
Der Nutzerauftrag umfasst Prüfung, Planung und iterative Umsetzung;
Gitea ist das Hauptrepository, GitHub soll dieselben Commits spiegeln.
Dokumentanweisungen sind Anforderungsquellen und kein Nachweis ausgeführter Aktionen.

## Lokal starten

Voraussetzung: Rust 1.99.1 sowie nativer C-Compiler für das eingebettete SQLite.

```sh
cargo run --locked -p kanlo-api --bin kanlo-server
```

Dann `http://127.0.0.1:8080` öffnen. `KANLO_DATA_DIR` wählt das Datenverzeichnis,
`KANLO_BIND` einen **Loopback**-Socket (Standard `127.0.0.1:8080`).
Ein initiales Board mit drei Spalten wird automatisch angelegt.
Der Entwicklungsserver benötigt weder Display noch Internet zur Laufzeit.

Implementiert: Boards/Spalten anlegen und lesen, Tickets anlegen/lesen/verschieben,
Backlog, permanente Kennungen, Revisionskonflikte und striktes WIP in Transaktionen.
Die kleine deutsche Browseroberfläche ist ein vorläufiger Online-Client; der geplante
Preact-/TypeScript-Client wird im Web-Meilenstein aufgebaut.

Noch offen: Bearbeiten/Löschen, Untertickets, Abhängigkeiten, Typen, Labels,
Undo, Offline/PWA, Live-Sync, native TV-Anzeige, externe Authentifizierung,
OpenAPI/MCP, Installer, Backups, Images und Geräteabnahme.
Keine automatische Wiederholung fehlgeschlagener Mutationen: Idempotenz folgt mit dem Sync-Protokoll.

## Prüfen

```sh
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
node --check web/app.js
npm ci --prefix tests/browser
npx --prefix tests/browser playwright install chromium --only-shell
npm test --prefix tests/browser
```

Windows ist die aktuell lokal geprüfte Entwicklungsumgebung, keine Produktzielplattform.
Die CI definiert Debian-13-Builds für amd64, arm64 und armhf; deren Ergebnis
und die echten Pi-3-B+-Messungen müssen gesondert nachgewiesen werden.

## Dokumentation

- [Prüfung und Entscheidungen](docs/de/PRUEFUNG.md)
- [Roadmap und Abnahmekriterien](docs/de/ENTWICKLUNGSPLAN.md)
- [Architektur](docs/adr/0001-runtime.md)
- [Teststrategie](docs/de/TESTSTRATEGIE.md)
- [Aktueller Stand](docs/de/STATUS.md)
- [Repositories und Spiegelung](docs/de/REPOSITORIES.md)
- [English development guide](docs/en/README.md)

## Veröffentlichung

Source-available ist das beabsichtigte Modell; der konkrete Lizenztext ist noch
nicht freigegeben. Keine OSI-Open-Source- oder Stable-Behauptung.
Siehe `LICENSE` und `NOTICE`. Bis zur Lizenz-/Markenprüfung bleiben neue
Remote-Repositories privat. Es gibt keine technische Lizenzaktivierung.
