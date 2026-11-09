# Teststrategie und Veröffentlichungsgates

## Lokal ausführbar

- `cargo fmt --all -- --check`: Format.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: Lint.
- `cargo test --workspace --locked`: Domain, SQLite und HTTP.
- `npm run build --prefix web`: TypeScript-Prüfung und lokaler Produktionsbuild.
- `npm test --prefix tests/browser`: Desktop-/Mobile-, Offline-, Mehrtab-,
  HTTPS-/Auth-, Epochen-, Filter-/Bulk- und Eingabeprüfungen mit realem Server.

DB-Tests prüfen Neustart/Instanz/Epoche/Zähler, Zukunftsschema, Integrität,
atomaren Fehlversuch, Done/Backlog, falsche Boardspalte, stabile ID nach Umzug,
stale Revision und zwei konkurrierende Verbindungen am WIP-Limit.
HTTP-Tests prüfen Roundtrip, Validierung, Host/Origin-/Forwarded-Header-Umgehung,
Bodylimit, unerlaubte Felder, Priorität und lokale Assets mit CSP.

## Abdeckung und weiterhin verpflichtende Suites

Domain: Typ-/Kind-/Abhängigkeitsregeln, WIP warning/strict, Undo=0, atomarer Batch.
Offline (implementierte Abnahme): unabhängige Felder/gleiche Felder, kausale Queue, temp IDs,
Idempotenz, Mehrtab, 401/403, Epochwechsel und Tombstones.
Browser (implementierte Abnahme): Smartphone/Desktop, DE/EN, Keyboard/Touch,
persistentes Modal, HTTPS/PWA mit im isolierten Browser gepinntem Testzertifikat,
Konfliktdialog, sichere Markdown-Ausgabe. TV-Effekte bleiben bis zum nativen
Renderer unprüfbar; öffentliche TLS-Zertifikatsketten sind ein Betriebsgate.
TV: max. 1/2 Overlays, Backlog-/Elternrückkehr, 2D-Scroll, Rotation-Pause,
Dirty Regions, Crash-Restore und Langzeitbetrieb.
Security: getrennte Listener, Proxy-Bypass, Sessions/Keys/CSRF/Rate Limits,
XSS/SSRF, Pfade, CSV-Formeln, kein Secret in Export/Backup/Logs/Cache.
Recovery: Onlinebackup, selektiver Restore, Migrationsabbruch, Rollback,
Datenortwechsel, Fresh-Flash und manuell angestoßenes Update.

## Debian- und Architektur-Abnahme reproduzieren

```sh
docker build -f packaging/tests/Dockerfile -t kanlo-validation:debian13 .
docker run --rm -v "$PWD:/src" -v kanlo-build:/build kanlo-validation:debian13 sh scripts/debian-validate.sh
docker run --rm --network none --user kanlo --cap-drop ALL -v "$PWD/artifacts:/artifacts:ro" -v "$PWD/packaging/tests:/tests:ro" kanlo-validation:debian13 python3 /tests/headless.py
```

PowerShell: Bind-Mount-Pfade absolut angeben. Der Build erzeugt Debug-ELF und
SHA256 für AMD64/ARM64/armhf und führt die AMD64-Rustsuite aus. Der zweite
Container hat kein Netzwerk außer Loopback, keine Anzeige und keinen Rootnutzer.
Er prüft Start/Neustart, Verzeichnisrechte, Health, Integrität und SIGTERM bei
offenem Ereignisstrom. Keine dieser Prüfungen ersetzt physische Pi-/SSD-Tests.

## Geräte-Messplan

Debian 13 amd64, Raspberry Pi OS/Debian 13 arm64 und armhf auf Pi 3 B+ soweit
jeweilige OS-/Hardwareunterstützung reicht. Messwerte: RSS/PSS, CPU idle/Animation,
API-/Renderlatenz p50/p95, SD-Schreibvolumen und Boot/Neustartzeit. Szenarien:
leeres Board, 1000+ Tickets, lange Markdown-Kommentare/Bilder, zwei Overlays,
2D-Scroll, Statistik, Backup und Dauerlast. Budgets erst aus realer Messung
ableiten; kein fiktiver Grenzwert und keine Pi-Kompatibilitätsbehauptung.

Dev: Teilumfang, Tests und offene Punkte benannt.
Beta: Kernfunktionen plus Geräte-/Recovery-/Security-Nachweise.
Stable: alle Baseline-Gates, Lizenz/Markenprüfung, reproduzierbare Artefakte,
SHA256 und DE-/EN-Anleitungen tatsächlich vorhanden.
