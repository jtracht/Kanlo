# Entwicklungsstand — 9. November 2026

**50 % des ursprünglichen Aufgabenbestands abgenommen: 35 von 70 Aufgaben.**
53 Rust- und 32 Browserprüfungen sowie alle fünf Jobs der
[Gitea-CI 787](https://git.tracht-digital.de/jtracht/Kanlo/actions/runs/787)
sind erfolgreich. Programm-/Prüfstand: `0c1cd59a7dd8685e5605116f758ad3e5c81a4ea8`.
Die abschließenden Dokumentationsänderungen ändern diesen Programmstand nicht.

Die Umsetzung auf `feature/fifty-percent` umfasst den Domainkern, sichere
HTTP-Zugänge, eine vollständige bearbeitbare Weboberfläche und die dauerhafte
Offline-Warteschlange. Der abschließende Abgleich der 70 ursprünglichen Aufgaben
steht in [50-PROZENT.md](50-PROZENT.md); nur vollständig belegte Aufgaben zählen.

## Nachgewiesener Funktionsumfang

- SQLite-Schemaversion 4 mit transaktionaler Migration, WAL/FK, Integritätsprüfung,
  eindeutigen Kennungen, privater Datenablage und Änderungsgruppen.
- Boards/Spalten, Farben/Done/WIP; Tickets/Untertickets, Typaktivierung und
  ausdrückliche Typkonvertierung beim Boardwechsel; Labels und eigene Textfelder.
- Zyklenverbot, Abschlussblockade und transitive Wiederöffnung mit Backlog-Fallback.
- Kommentarmodell/Links, Markdown-Sanitizing, Suche, strukturierte Filter,
  gewichteter Score und vom Sortierwechsel unabhängiger manueller Rang.
- Papierkorb mit Kaskade, bestätigungspflichtige Vorschau, konfliktgeprüfte
  Wiederherstellung; Archiv und standardmäßig ausgeschaltete Fristen.
- Persistentes Undo 0–1000, gruppierte/mehrschrittige Rücknahme mit Vorschau;
  Statistik und Papierkorb funktionieren bei Undo=0 weiterhin.
- Wiederholung mit Zeitzonen/DST, Wochen-/Monatsregeln und „erst nach Abschluss“.
- Versionierte JSON-Roundtrips, eingeschränktes/formelgeschütztes CSV,
  Importvorschau, konsistente Datenbackups ohne Auth-Geheimnisse, Sicherheitskopie
  vor vollständigem/selektivem Restore und neue Datenepoche.
- Getrennte LAN-/externe Router, keine Autorisierung durch Forwarded-Header;
  Argon2id, Secure/HttpOnly/SameSite-Cookies, CSRF, Rate Limit, widerrufbare API-Keys.
- Preact/TypeScript mit DE/EN, Themes, Touch/Drag/Tastatur, persistenten Ansichten,
  sichtbaren Lade-/Fehler-/Synczuständen und Undo-Toast nur bei aktivierter Historie.
- IndexedDB-Queue mit stabilen op_ids, kausalen temporären IDs, Mehrtab-Koordination,
  Three-Way-Merge und expliziten Konflikt-/Epochenentscheidungen. Bestätigte
  Entitätsänderungen werden vor weiteren Netzabrufen atomar lokal gespeichert.
- Metadaten-Ereignisstrom, Browser-Reconnect; Webmodal behält ungespeicherte
  Eingaben bei externen Änderungen. Keine TV-Laufzeit wird vorgetäuscht.

## Prüfungen und Grenzen

Die aktuellen maschinenlesbaren Nachweise und Testergebnisse liegen unter
`docs/validation/`. Browserfälle prüfen Desktop 1440×1000 und mobiles Chromium
390×844, Offline-Neuladen, Mehrtab, gleiche/unabhängige Felder, Restore-Epoche,
HTTPS-PWA, Session-Widerruf, gespeicherte Ansichten, Bulk-Vorschau, DE/EN und Undo=0.
Der isolierte HTTPS-Browser vertraut ausschließlich dem Testzertifikat über
seinen SPKI-Pin; das ist kein Nachweis einer öffentlich ausgestellten Zertifikatskette.

Debian 13 wurde im Container ohne Netzwerk, Anzeige und zusätzliche Fähigkeiten
unter dem Dienstnutzer `kanlo` gestartet, neugestartet und beendet. Offene SSE-
Verbindungen verhindern den Shutdown nicht. AMD64/ARM64/armhf-Debug-Builds werden
mit Zielarchitektur und SHA256 geprüft. Docker ist nur eine Testumgebung und
keine Produktabhängigkeit.

Keine reale Pi-Hardware, kein TV-Renderer, kein Geräte-Boot/Kiosk, kein SSD-
Hardwaretest, kein fertiger Installer/Updater, kein vollständiges OpenAPI/MCP,
keine öffentliche App-Listung oder Live-Voice-Abnahme. Das Laden des Domain-
Aggregats in RAM bleibt ein ausdrücklich offenes Pi-Ressourcenrisiko.

Frühere CI-Nachweise bleiben als Historie erhalten, gelten aber nur für ihre
jeweiligen Commits. Neue Abnahmen werden mit eigener Commit-ID dokumentiert.
