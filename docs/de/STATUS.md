# Entwicklungsstand — 9. November 2026

## Tatsächlich vorhanden

- Drei Originaldokumente unverändert übernommen, Konsistenzprüfung und Roadmap.
- Privates Gitea-Repository und privates GitHub-Repository angelegt.
- Elf Meilensteine M0–M10 und 70 offene Issues in Gitea erstellt.
  Zuordnung mit echten Remote-Nummern unter `docs/planning/remote-state.json`.
- Rust-Workspace mit Domainverträgen, SQLite-Mutation-Service und HTTP-Adapter.
- Migration, WAL/FK, Instanz/Epoche, Seed, permanente Ticketnummern.
- Boards/Spalten/Tickets anlegen/lesen und revisionsgeprüfter Ticketumzug,
  Done/Backlog, transaktionale strikte WIP-Sperre auch bei konkurrierenden DB-Verbindungen.
- Provisorischer deutscher Online-Browserclient; ausschließlich Loopback.
- Lokale Assets, CSP, Host-/Origin-Prüfung, Bodylimit, systemd-Entwurf.
- GitHub-CI-Definition für Qualität, Browser und Debian-13-Buildmatrix
  amd64/arm64/armhf. Noch kein Stable-Release.

## Nachgewiesen lokal

Windows, Rust/Cargo 1.99.1, Node 24.16.0:

- `cargo test --workspace --locked`: 13 Tests bestanden.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: bestanden.
- `cargo fmt --all -- --check`: bestanden.
- `cargo build --workspace --locked`: bestanden.
- `node --check web/app.js`: bestanden.
- `npm test --prefix tests/browser`: zwei Chromium-Tests bestanden,
  Desktop 1440×1000 und mobiles Layout 390×844. Erstellen, Verschieben,
  Neuladen/Persistenz, HTML als Text, keine JS-Fehler/kein Seitenüberlauf.
- Laufender HTTP-Server: Health und tatsächliche Ticketanlage erfolgreich.

Die integrierte Browseransicht konnte wegen fehlender Laufzeitdateien nicht
gestartet werden; die gesonderten Playwright-Tests liefen anschließend erfolgreich.
Vorhandene DB-Tests belegen Neustartpersistenz zusätzlich zum Browser-Neuladen.

## Offen

M0 ist vorbereitet; CI/Branchregeln/Lizenzfreigabe noch separat abzunehmen.
M1/M2/M3 sind begonnen, keine dieser Gesamtphasen ist fertig. Alle 70 Issues
bleiben offen. Vollständiger Domainkern, Editieren/Löschen, Offline/Sync,
TV-Renderer, Preact/TypeScript und DE/EN, Auth/MCP/OpenAPI, Backup/Restore,
Installer/Updater/Image und Release-/Hardwareabnahmen fehlen.
Keine Pi-Hardware-/Debian-Laufzeitmessungen, kein externer Auth- oder Live-Voice-Test.

## Remote-Nachweise

Erster Implementierungscommit `06f21598021e4472a4ac5359519a027d96e4a63c`
auf Gitea und GitHub gepusht; beide Branch-IDs per `git ls-remote` abgeglichen.
GitHub-Lauf: https://github.com/jtracht/Kanlo/actions/runs/42575662919.
Qualitätsjob inklusive Rust- und Browsertests erfolgreich. ARM32/ARM64-Builds
meldeten fehlende Cross-libc-Header beim SQLite-Build. Die Matrix wurde um
`libc6-dev-armhf-cross`/`libc6-dev-arm64-cross` ergänzt; erneute Abnahme steht aus.
