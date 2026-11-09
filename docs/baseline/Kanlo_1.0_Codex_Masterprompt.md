# KANLO 1.0 – CODEX-MASTERPROMPT

> **Zur Verwendung:** Diesen Prompt an Codex übergeben, zusammen mit `Kanlo_1.0_Technische_Spezifikation.md` und optional `Kanlo_1.0_GitHub_Issues.md`. Die beiden Begleitdateien sind die fachlich verbindliche Detailbasis. **Kein Auftrag zur Behauptung bereits ausgeführter GitHub-, Test- oder Release-Aktionen.**

## ROLLE UND MISSION

Du agierst als Senior Softwarearchitekt, Lead Rust-/TypeScript-Entwickler, Embedded-Linux-/Grafikexperte, API-Security-Engineer, Testarchitekt und technischer Projektmanager.

Entwickle **Kanlo 1.0** vollständig und produktionsfähig: eine extrem ressourcenschonende, selbst gehostete Kanban-Anwendung, deren einziger Server auf Raspberry Pi OS 13 beziehungsweise Debian 13 (ARM32/armhf, ARM64/aarch64, AMD64/x86-64) läuft. Der lokale Fernseher wird vom **selben Gerät** versorgt wie Webserver, SQLite-Datenbank und REST/OpenAPI. Die TV-Anzeige läuft je nach Installation automatisch als schlanker Kiosk oder manuell auf einem unverändert bleibenden Debian-Desktop. Ohne Display funktionieren Server, API und Website normal weiter. Smartphones/PCs sind nur Browserclients, keine separaten Server.

**Verbindliche Eingaben und Reihenfolge:**
1. Lies die gesamte technische Spezifikation und den Issue-Backlog, falls mitgegeben.
2. Erkunde tatsächlichen Repository-/Datei-/CI-/Hardwarezustand; erfinde nichts.
3. Übernehme alle dokumentierten Produktentscheidungen als harte Anforderungen. Wenn eine technische Wahl nicht feststeht, triff eine begründete, wartbare Standardentscheidung selbst.
4. Stelle nur dann eine Rückfrage, wenn ein nicht auflösbarer, sicherheits- oder produktstrategisch entscheidender Widerspruch besteht. Keine Mikromanagement-Fragerunde.
5. Plane Meilensteine und Tickets, dann implementiere iterativ bis zur maximal wirklich testbaren Vollständigkeit. Jede Aussage „fertig“ muss durch konkrete Ergebnisse und Tests belegt sein.

## NICHT VERHANDELBARE PRODUKTREGELN

### System und Distribution

- Ein lokaler Kanlo-Server mit SQLite, optionalem direkt angeschlossenem HDMI-/Monitor-Display, eigenem lokalem TV-Prozess und Webserver.
- Native systemd-Dienste **ohne Docker-Zwang**; `install.sh` plus flashbares Raspberry-Pi-Image. Zwei Modi: **Kiosk** oder **Desktop-Integrationsmodus**. Headless-Fähigkeit. ARM32/ARM64/AMD64.
- Keine eigenen `.deb`-Releasepakete oder eigenes APT-Repository in 1.0.
- Versionierte GitHub Releases mit überprüfbaren Installations-/Programm-/Image-Artefakten. GitHub Actions baut, testet und verpackt.
- Updates ausschließlich auf Benutzerbefehl (SSH/Skript, Web, ChatGPT), mit Backup/Migrationskontrolle/Rollback, niemals automatische Updateprüfung oder Installation.
- Nach Abstürzen systemd-Neustart betroffener Dienste, kein eigener hochkomplexer Watchdog; TV-Zustand restaurieren.
- RAM/CPU/Render-Latenz am echten **Raspberry Pi 3 B+** messen und optimieren, nicht nur im Desktop-Simulator.

### Web- und TV-UI

- Mobile-first Website und installierbare HTTPS-PWA mit **vollständiger Offline-Bearbeitung bereits besuchter Boards**. Browser hält Offline-Queue und Cache, aber keine autoritative Serverdatenbank. Ohne serverseitige Auth wird eine extern abgelaufene Sitzung **nicht** umgangen; Änderungen bleiben bis zur erneuten Anmeldung.
- Lokaler Online-Zugriff über HTTP (z. B. LAN-IP) **ohne Web-Login**. Externer Browserzugriff nur über Reverse Proxy mit HTTPS und **von Kanlo geprüftem gemeinsamen Zugangscode**. Sichere Ingress-Trennung, nicht auf clientgegebene Forwarded-Header vertrauen.
- **PWA auf fremden Geräten erfordert in der Regel vertrauenswürdiges HTTPS**; LAN-HTTP bleibt normale Online-Website, aber keine falschen PWA-/Offline-Versprechen.
- Eigener Splashscreen und eigener Ladescreen in Website/TV, ohne künstliche Wartezeit.
- TV zeigt passive Kanban-Boards, globale Gesamtübersicht und separate TV-Statistikseite. Keine TV-Vorschau/Spiegelung in Website, kein Bildschirm-Standby/Abdunklung/Bildschirmschoner.
- Quer-/Hochformat, Ultrawide, kleine und ungewöhnliche Displays; flüssige, ressourcenadaptive UI; Dark/Light/individuelle Farben.
- Animationsprofile **Aus, Dezent, Flüssig (Default), Dynamisch**; alle wesentlichen Effekte im MVP.
- Auto-Scroll nur TV: bei 2D-Überlauf zuerst vertikal, jede sichtbare Spalte nacheinander vollständig, anschließend horizontal; auf aktive Anzeige/Overlay beschränkt, manuelle Steuerung pausiert/resetet korrekt.
- Optionaler TV-Ansichtswechsel zwischen frei wählbaren Boards, globaler Übersicht und Statistiken (Konfiguration/Intervall/Reihenfolge), Default aus; Overlay pausiert Rotation.
- TV-Ticket-Overlay: max. zwei bei ausreichendem Platz, sonst eins, drittes ersetzt ältestes; Timeout 0–300 Minuten, 0 dauerhaft; Neustart stellt Overlays wieder her und beginnt Timer erneut.
- Backlog-Overlay verdrängt Ticket-Overlays und stellt sie später wieder her; Ticket aus Backlog kehrt zum Backlog einschließlich Scrollposition zurück; Unterticket aus Elternticket kehrt nach Schließen ins Elternoverlay zurück; übrige Sonderregeln aus Spezifikation beachten.
- Web-Ticketmodal wird **niemals** automatisch geschlossen, und Web-Öffnen verändert den TV nicht. Kontextaktionen Löschen, Klonen, „TV anzeigen“, Details.
- Mehrfach-Highlights, Scroll-to-Ticket ohne Overlay, lesbare TV-Suchgruppen bei mehrdeutiger Suche, persistente Filter/Sortierung getrennt je Web-/TV-Kontext.
- Grundlegende Tastaturbedienung, Lesbarkeit/Kontrast; WCAG-AA nicht als bereits garantiert ausgeben.

### Fachliches Datenmodell

- Beliebig viele Boards, initial eines; konfigurierbare Spalten inkl. ein/mehrerer Done-Spalten, Backlog, WIP-Warnung oder pro Spalte strikte Sperre.
- Boardkürzel und Ticketnummer: z. B. `DEV-42` als **permanente Kennung**, auch nach Verschieben auf andere Boards. Interne UUID ergänzend. Kein ID-Recycling.
- Tickettypen optional, globale Typbibliothek pro Board aktivierbar, Priorisierungsrang, Farbe/Icon, Unterticket-Erlaubnis. **Typen definieren keine Custom Fields und keine typabhängigen Pflichtfelder.**
- Tickets haben beliebige **eigene Name/Textwert-Custom-Fields**, unabhängig vom Tickettyp; beim Typwechsel bleiben sie erhalten.
- Labels ausschließlich **global**, farbig und mehrwertig auf jedem Board; nicht auf Boards einschränken.
- Priorität 0–255, Typenrang und Datum/Uhrzeit-Deadline als konfigurierbare Einflussgrößen für Gesamtscore; Sortierung/Filterung nach allen sinnvollen Eigenschaften, zusätzliche dauerhaft gespeicherte manuelle Reihenfolge.
- Untertickets genau eine Ebene, binär fertig/nicht fertig, nicht unabhängig zwischen Boards/Spalten bewegen. Eltern und alle Kinder ziehen beim Boardwechsel zusammen um. Eltern abgeschlossen ⇒ Kinder automatisch abgeschlossen. Eltern wieder geöffnet ⇒ Kinder grundsätzlich fertig lassen.
- Strikte Abhängigkeiten boardübergreifend und azyklisch. Abschluss verboten, solange Voraussetzungen offen. Wiederöffnen eines Vorgängers ⇒ transitiv abhängige erledigte Tickets wieder öffnen. Letzte aktive Spalte verwenden, sonst Backlog, insbesondere bei strengem WIP-Limit.
- Bei Boardwechsel fehlenden Tickettyp nicht still ersetzen: **Typ auf Zielboard aktivieren oder ausdrücklich konvertieren**; Zielspalte/Backlog auswählen, gleichnamige Spalte vorschlagen.
- Löschen eines Eltern-Tickets ⇒ Kinder in Papierkorb, Abhängigkeiten auflösen, vorher Folgen anzeigen; Papierkorb mit Wiederherstellung und optionaler automatischer Löschung.
- Ändern/Entfernen von Boards/Spalten nur, wenn komplett leer. Typen nur entfernen, wenn keine Tickets mehr darauf verweisen.
- Undo-Historie pro Board (0–1000, Default 100), persistent und für Web/ChatGPT nutzbar; 0 deaktiviert Undo komplett, Papierkorb bleibt. Massenaktionen gruppiert und konsistent rückgängig machen.
- Markdown-Beschreibungen und separate Markdown-Kommentare mit Zeitstempel; verlinkte Bilder in TV-Ticketdetails, kein lokaler Dateiupload.
- Optionale Archivierung manuell und nach einstellbarer Zeit; Wiederholungstickets mit flexiblen Regeln inklusive „erst nach Abschluss“; keine generische Workflow-Engine.
- Batch-Änderungen nach Mehrfachauswahl oder Such-/Filterausdruck, Vorschau und Bestätigung vor großen/destruktiven Änderungen; dieselben Domainconstraints wie Einzeltickets.
- Historische Basisdiagramme (erstellt, abgeschlossen, Backlog, blockiert), frei wählbare Zeiträume; Web/ChatGPT/TV-Statistikansicht; Statistikdaten unabhängig von begrenzter Undo-Historie halten.
- Keine Zeittrackingfunktion, Kalenderansicht, Swimlanes, Boardklone oder Boardvorlagen, Push-Notifications, Outgoing Webhooks, eingebaute Drittanbieter-Plugins.

### API und ChatGPT

- Eine zentrale **versionierte REST/OpenAPI-API** mit allen Funktionen und Einstellungen. Web/MCP/GPT Actions dürfen keine getrennten Geschäftsregeln besitzen.
- Widerrufbare, getrennte API-Schlüssel, sichere Hashspeicherung und kein Secret-Export in Backups. Geführte und manuelle MCP-/GPT-Actions-Einrichtung, unabhängige Verbindungstests.
- Selbst eingerichtete volle Integration muss funktionieren. Zusätzlich öffentliche ChatGPT-Integration vorbereiten/einreichen, **sofern Plattform/Review dies zulassen**.
- ChatGPT-App bietet Spracheingabe, aber MCP-/Action-Aufrufe aus Live-Voice sind nicht pauschal garantiert. Diktierten Text, Textaufruf und Live-Voice gesondert prüfen; nichts versprechen, was nicht in Tests funktioniert.
- Kontextuelle Suche: explizite IDs vorrangig, sonst sichtbare Tickets, dann Offscreen/Backlog und andere Boards, Mehrdeutigkeiten transparent auflösen.
- Spracheinstellungen: Kategorien zuerst, Unterkategorien/Settingnamen auf Anfrage, Werte nur ausdrücklich; lange Listen in Fünferblöcken mit Fortsetzungszustand.
- Sicherheit bei ChatGPT-Schreibaktionen: API-Auth, bestätigungspflichtige destruktive Vorgänge, Änderungsverlauf, idempotente Aufrufe.

### Synchronisation und Backups

- Jede Kanlo-Mutation serverseitig validiert, transaktional und über denselben Kern verarbeitet. Feldbasierter **Drei-Wege-Abgleich** bei Offline-Queues; unabhängige Änderungen zusammenführen, echte Konflikte anzeigen, niemals blind „last write wins“.
- Übertragene Operationen mit Idempotenz-ID, Basisrevision, Basissnapshot/-feldern, Client-ID, Serverinstanz/Epoche. Browser-Queue persistent, pro Tab koordiniert, temp Ticket-IDs serverseitig mappen.
- Backup-Restore (komplett/selektiv) erhöht Datenepoche. Alte Offline-Queues müssen danach einzeln auf Validität/Konflikte geprüft werden; keine stillschweigende Wiederbelebung gelöschter Daten.
- Backups manuell/automatisch, SQLite-konsistent, **unverschlüsselt**, aber ohne API-Schlüssel, externe Session-Geheimnisse oder vergleichbare Secrets. Wiederherstellung mit Vorschau, ausdrücklicher Bestätigung, Sicherung vor Überschreiben.
- Speicherort für Daten/Backups frei wählbar und später sicher migrierbar, Default Systemlaufwerk.
- Boardexport: volles versioniertes JSON (Spalten, Tickets, Kindtickets, Kommentare, globale Labelzuordnung, Textfelder, Links, Abhängigkeiten, nötige Typdefinitionen), CSV nur sinnvoll tabellarische Daten. Keine Undo-Historie in normalem JSON-Export.
- CSV-/JSON-Import mit ID-Abgleich, Preview, Bestätigungswahl Aktualisieren/Überspringen/als neues Ticket; Standard Nichtüberschreiben.

### Datenschutz, Lizenz und Grenzen

- Keine automatische Telemetrie, Analytics an Dritte, Crashberichte, Tracking, externen CDNs. Diagnose nur lokal/manueller Export.
- Source-available-Lizenz: privat/nichtkommerziell kostenlos, kommerziell nur mit Erlaubnis, keine unabhängige Distribution modifizierter Versionen; **keine technische Lizenzaktivierung**. Konkrete Rechtsklauseln, Abhängigkeiten, Lizenzkompatibilität und Markenname vor Veröffentlichung prüfen. Nicht als OSI-Open-Source bezeichnen.
- Geheimnisse nie in HTML-Bundles, Offline-Caches, Logs, normalen Boardexporten oder unverschlüsselten Backups.
- Markdown sanitizen, API-/CSRF-/SSRF-/Host-/Rate-Limit-Schutz, CSV-Formel-Injection entschärfen, Pfad-/Dateiupload-Sicherheit, DB-Migrationsschutz.

## TECHNISCHER UMSETZUNGSVORSCHLAG

Bevorzugte Struktur (bei echter Notwendigkeit begründet modifizierbar):

```text
kanlo/
├── crates/
│   ├── kanlo-core/         # Domain, Validatoren, Befehle, Revisionen, Sync/Undo
│   ├── kanlo-db/           # SQLite-Schema, Migrationen, Backup/Restore
│   ├── kanlo-api/          # REST/OpenAPI, Auth, SSE/WS, MCP-Adapter
│   ├── kanlo-tv/           # nativer TV-Renderer, Overlays, Scroll/Animation
│   └── kanlo-cli/          # Installer-/Support-/Migrationsbefehle
├── web/                    # Preact/TS/Vite, PWA/IndexedDB/Offline-Konflikte
├── integrations/
│   ├── chatgpt-mcp/        # Beschreibung, Tools/Adapter/Anleitung
│   └── gpt-actions/        # OpenAPI-/GPT-Actions-Vorlagen
├── packaging/
│   ├── install.sh
│   ├── systemd/
│   ├── image/
│   └── update.sh
├── docs/de/  docs/en/
├── tests/                  # E2E, Sicherheits-, Architektur- und Hardwaretests
├── .github/workflows/
├── LICENSE  NOTICE  README.md
└── CHANGELOG.md
```

Empfohlen: Rust/Axum/Tokio/SQLite-Backend und nativer SDL2-/Pango-/Cairo-TV-Renderer, leichte TypeScript/Preact-PWA. Prüfe sämtliche Bibliotheken für Debian 13 und ARM32 einschließlich glibc, EGL/KMS/Displayrechten. Priorisiere reproduzierbare Offline-Builds und kleine Ressourcenbudgets. Ein vollständiger Browser als TV-Runtime darf nur nach echten Pi-3-B+-Benchmarks gewählt werden.

Vorgeschlagene HTTP-Verträge (genaues API-Design aus Domänenmodell ableiten):

- `/api/v1/boards`, `/boards/{id}/columns`, `/boards/{id}/tickets`
- `/api/v1/tickets/{id}`, `/tickets/{id}/children`, `/tickets/{id}/comments`, `/tickets/{id}/dependencies`, `/tickets/{id}/labels`
- `/api/v1/tickets/bulk/preview` und `/api/v1/tickets/bulk/execute`
- `/api/v1/ticket-types`, `/api/v1/labels`, `/api/v1/search`
- `/api/v1/views/{view_id}/state`, `/api/v1/displays`, `/api/v1/displays/{id}/commands`
- `/api/v1/settings/categories`, `/api/v1/settings/{key}`
- `/api/v1/undo`, `/api/v1/trash`, `/api/v1/archive`
- `/api/v1/recurrence`, `/api/v1/statistics`, `/api/v1/imports`, `/api/v1/exports`
- `/api/v1/backups`, `/api/v1/backups/restore/preview`, `/api/v1/backups/restore/confirm`
- `/api/v1/sync/handshake`, `/api/v1/sync/operations`, `/api/v1/sync/conflicts`
- `/api/v1/events` (SSE/WS), `/api/v1/integrations/chatgpt/test`

MCP und GPT Actions sollten gemeinsame Domain-Services über definierte API-Adapter verwenden, keine eigenständige Implementierung der Regeln.

## QUALITÄTSSICHERUNG UND RELEASE-GATES

**Tests sind nicht optional; sie dürfen nach einem sinnvoll zusammenhängenden Implementierungsschritt konzentriert ausgeführt werden, statt jedes Tokens mit Mini-Tests zu verbrauchen.** Belege bei jeder Phase tatsächliche Ergebnisse.

1. Unit-/Property-Tests: Priorisierung, Sortierpersistenz, Datums-/Zeitzonenregeln, Wiederholungspläne, Abhängigkeitszyklen, Transaktions-Rollbacks, undo=0, Kindticketstatus, WIP-Modes.
2. Integrations-/DB-Tests: simultanes Web/API-Update, Backup-Restore/selektive Wiederherstellung, Ticket-/Typ-/Board-Deletion, Im-/Export-Roundtrip, SQLite-Integrity, Migration.
3. Offline-E2E: zwei Clients offline mit Änderungen an verschiedenen Feldern ⇒ Merge; gleiches Feld ⇒ sichtbarer Konflikt; Session abgelaufen ⇒ Queue bleibt; Backup-Restore ⇒ Epochenerkennung; gelöschte Datensätze nicht still wiederauferstehen.
4. Web-E2E: Smartphone/PC, Touch/Keyboard, Backlog, Modal ohne TV-Wirkung, Dark/Light, DE/EN, Offline-Splash/Ladescreen, PWA auf echtem HTTPS.
5. TV-E2E: unterschiedliche Displayformate, max. 2 Overlays, Backlog-Rückkehr, Kindticket-Rückkehr, Auto-Scroll 2D, Rotation pause, Langzeitbetrieb, Wiederherstellung nach Crash.
6. API/ChatGPT: alle Domainfunktionen, Wertelesen/Editieren der Einstellungen, Auth-Revoke, Mehrdeutigkeitsabfragen, Fünferlisten, Live-Sync, destruktive Vorschauen. Text und Voice-Möglichkeiten separat und nachweisbar testen.
7. Security: LAN ohne Auth versus externer Proxy mit Kanlo-Code, fehlendes/falsches Cookie, manipulierte Header, CSRF/XSS/SSRF, Rate Limits, Secret-Leaks, CSV-Formeln, Datenzugriff außerhalb definierter Verzeichnisse.
8. Reale Geräte: Pi 3 B+ mit armhf und arm64 soweit OS/Hardware unterstützt; amd64 Debian; KMS/DRM-Grafik, HDMI, Bedienung, Boot, Reboot, SD-Schreibverhalten, Ressourcenbudget.
9. Update-/Restore-Tests: vor Update Backup; unterbrochene Migration, Rollbackpfad, Datenortwechsel, frisch geflashtes Pi-Image.
10. Releaseartefakte: Checksummen, Build-Metadaten, README mit exakten Bedingungen, Lizenzprüfung, Release-Notes, bekannte Einschränkungen. Nie erfundene Benchmarks oder „voll kompatibel“-Versprechen.

## VORGEHEN IN PHASEN

Setze die Meilensteine **M0–M10** der Spezifikation als Issues/Milestones um. Schreibe pro Issue in GitHub eine reproduzierbare Schritt-für-Schritt-Checkliste, Akzeptanzkriterien, Abhängigkeiten und Tests. Falls GitHub-Zugriff nicht vorhanden ist, erstelle die Artefakte lokal im Repository, statt behaupten, remote Issues/Releases erstellt zu haben.

### Phasenroutine

1. Bestand prüfen: Repository, Branches, Codestruktur, CI, dokumentierte Entscheidungen und offene Tickets.
2. Nächste nach Abhängigkeiten sinnvolle Gruppe von Issues auswählen; auf kleine überprüfbare Einheiten schneiden.
3. Minimalen kohärenten vertikalen Schnitt implementieren, der sich bauen, starten und auf Fehler prüfen lässt.
4. Sicherheits- und Architekturregeln unabhängig vom UI erzwingen.
5. Dokumentation/Tests und Akzeptanzkriterien aktualisieren; Commits/Pull-Requests erstellen, soweit erlaubt.
6. Reale noch offene Blocker und ungetestete Ziele als Issues dokumentieren; nicht als erledigt markieren.
7. CI, Versionierung, Releases und Hardwaretests erst als abgeschlossen melden, wenn Log-/Artefakt-/Gerätenachweise vorliegen.

**Release-Strategie:** Inkrementelle Dev-/Beta-Releases sind erlaubt und erwünscht; Stable 1.0 nur nach vollständiger Abnahme der vereinbarten Features. Eine noch nicht freigegebene öffentliche ChatGPT-Verzeichnislistung wird als externe Plattformabhängigkeit transparent geführt und darf die selbst eingerichtete Funktion nicht blockieren.

## AUSGABE UND STATUSBERICHT

Berichte in kompakten, überprüfbaren Fortschrittsabschnitten:

- Was ist **tatsächlich implementiert** (Dateien, Commits, Tests)?
- Was ist **nachweislich getestet** (Plattform, Kommandos/Ergebnisse)?
- Was ist **offen oder blockiert** und warum?
- Welche Issues/Milestones wurden real erstellt/geschlossen, welche lediglich geplant?
- Welche Änderungen wurden bewusst **nicht** durchgeführt, weil Rechte, Plattform/API oder Hardware fehlten?

**Starte jetzt mit M0, der Repository-/Architekturinspektion, den GitHub-Milestones/Issues sofern berechtigt und dem ersten testbaren Runtime-Schnitt. Setze dann nach Plan fort, statt beim Verfassen eines Konzepts stehenzubleiben.**
