# Kanlo 1.0 – GitHub-Milestones und Issue-Backlog

**Stand:** 8. November 2026  
**Status:** Planungsartefakt. Die hier aufgelisteten GitHub-Issues/Milestones wurden **nicht** automatisch auf GitHub angelegt. Issue-Nummern `KAN-...` sind Planungs-IDs und keine GitHub-Issue-Nummern.

Dieses Backlog gehört zu `Kanlo_1.0_Technische_Spezifikation.md` und zum `Kanlo_1.0_Codex_Masterprompt.md`. Für jede Aufgabe sind Arbeitsschritte und ein prüfbares Akzeptanzkriterium angegeben. Die genaue Umsetzung sollte in Abhängigkeitsreihenfolge erfolgen; Tests/Reviews werden nicht durch reine Aufgabenstatus-Aussagen ersetzt.

## Milestones

| Milestone | Name | Geplante Issues |
|---|---|---:|
| M0 | Repo, Lizenzen, CI | 5 |
| M1 | Server, SQLite, Installer | 6 |
| M2 | Kanban-Domänenkern | 9 |
| M3 | Responsive Weboberfläche | 6 |
| M4 | Native TV-Laufzeit | 8 |
| M5 | Offline-PWA und Live-Sync | 6 |
| M6 | Ticketfunktionen und Automatik | 7 |
| M7 | Statistik, Backup und Datenportabilität | 5 |
| M8 | Sicherheit und ChatGPT | 8 |
| M9 | Releasepakete und OS-Image | 5 |
| M10 | Abnahme und Stable 1.0 | 5 |

**Gesamt:** 70 geplante Issues.

## Issues und Arbeitsschritte

### KAN-001 · Repository-Struktur und Entscheidungskatalog

- **Milestone:** M0 – Repo, Lizenzen, CI
- **Umsetzung:**
  1. Repository und Branch-Regeln prüfen.
  2. Monorepo-Struktur für Rust/Web/TV/Packaging anlegen.
  3. Fachentscheidungen und Nichtziele im Docs-Verzeichnis versionieren.
- **Akzeptanz / Definition of Done:** README und Spezifikation sind im Repository; kein bestehender Code überschrieben.

### KAN-002 · Source-available-Lizenz und Abhängigkeitsprüfung

- **Milestone:** M0 – Repo, Lizenzen, CI
- **Umsetzung:**
  1. Konkretes Lizenzmodell in Platzhalter-Rechtsprüfung markieren.
  2. Drittanbieter-Lizenzen und Markenrisiken inventarisieren.
  3. NOTICE und Veröffentlichungscheckliste erstellen.
- **Akzeptanz / Definition of Done:** Keine fälschliche Open-Source-Behauptung; Freigabebedarf dokumentiert.

### KAN-003 · CI für ARM32, ARM64 und AMD64

- **Milestone:** M0 – Repo, Lizenzen, CI
- **Umsetzung:**
  1. Format- und Lint-Jobs anlegen.
  2. Cross-Build oder native Build-Matrix pro Zielarchitektur ergänzen.
  3. Buildartefakte mit Plattformkennzeichnung prüfen.
- **Akzeptanz / Definition of Done:** CI kann reproduzierbare Debug-Builds für alle Zielarchitekturen erstellen.

### KAN-004 · API-, Datenbank- und UI-Architektur dokumentieren

- **Milestone:** M0 – Repo, Lizenzen, CI
- **Umsetzung:**
  1. ADR für einen Server und zwei Oberflächen verfassen.
  2. API-/Domain-Module voneinander trennen.
  3. Risikoregister Pi3, HTTPS/PWA, ChatGPT anlegen.
- **Akzeptanz / Definition of Done:** Technikentscheidungen sind nachvollziehbar und testbar begründet.

### KAN-005 · Teststrategie und Qualitätsbudget

- **Milestone:** M0 – Repo, Lizenzen, CI
- **Umsetzung:**
  1. Unit-/Integrations-/E2E-Ebenen definieren.
  2. Pi-3-B+-Benchmarkfälle und Hardwarematrix spezifizieren.
  3. Abnahmekriterien für Dev/Beta/Stable dokumentieren.
- **Akzeptanz / Definition of Done:** Messplan ohne unbelegte Performanceversprechen vorhanden.

### KAN-006 · Serverstart und Konfigurationssystem

- **Milestone:** M1 – Server, SQLite, Installer
- **Umsetzung:**
  1. Minimalen Rust-HTTP-Dienst mit Settings laden.
  2. Fehlermeldungen und lokale Logs implementieren.
  3. Headless-Start unter Debian 13 testen.
- **Akzeptanz / Definition of Done:** Server startet ohne HDMI und ohne Internet.

### KAN-007 · SQLite-Schema, Migrationen und sichere Dateirechte

- **Milestone:** M1 – Server, SQLite, Installer
- **Umsetzung:**
  1. Schema-/Migrationrunner anlegen.
  2. WAL, FK und Transaktionskontrolle aktivieren.
  3. Systemnutzer und private Datenverzeichnisse konfigurieren.
- **Akzeptanz / Definition of Done:** Schema migrationsfähig; sqlite integrity check grün.

### KAN-008 · systemd-Units und Neustartverhalten

- **Milestone:** M1 – Server, SQLite, Installer
- **Umsetzung:**
  1. Kanlo-Server-Unit mit Startlimit entwerfen.
  2. TV-Unit separat und optional einrichten.
  3. Absturz- und Neustarttests durchführen.
- **Akzeptanz / Definition of Done:** Server und TV-Prozess werden unabhängig restartfähig.

### KAN-009 · Installationsskript mit Kiosk-/Integrationsmodus

- **Milestone:** M1 – Server, SQLite, Installer
- **Umsetzung:**
  1. Distro/Architektur erkennen.
  2. Kiosk- und Desktop-Integrationspfade implementieren.
  3. Uninstall/Upgrade-/Rollback-Vorbereitung hinzufügen.
- **Akzeptanz / Definition of Done:** Desktop bleibt in Integrationsmodus unverändert; Kiosk startet gezielt.

### KAN-010 · Display-Erkennung und Headless-Start

- **Milestone:** M1 – Server, SQLite, Installer
- **Umsetzung:**
  1. Angeschlossene Displays zuverlässig ermitteln.
  2. TV-Unit bedarfsabhängig aktivieren.
  3. Nachträgliches Display und Reboot testen.
- **Akzeptanz / Definition of Done:** Kein Display blockiert weder Backend noch Website.

### KAN-011 · Einrichtungsassistent und Netzwerk-Grundsetup

- **Milestone:** M1 – Server, SQLite, Installer
- **Umsetzung:**
  1. Setup-URL und QR am TV beim ersten Start anzeigen.
  2. LAN/WiFi erkennen und optional zeitlich begrenzten gesicherten Setup-Hotspot bereitstellen.
  3. Nach Ersteinrichtung erstes Board automatisch anzeigen.
- **Akzeptanz / Definition of Done:** Setup funktioniert auf Smartphone/PC und schaltet Hotspot wieder ab.

### KAN-012 · Boards und Spalten mit Backlog

- **Milestone:** M2 – Kanban-Domänenkern
- **Umsetzung:**
  1. Board-CRUD und eindeutige Kürzel implementieren.
  2. Spaltenreihenfolge, Farben und Done-Kennzeichnung anbieten.
  3. Leere-Löschbedingungen inklusive versteckter Tickets testen.
- **Akzeptanz / Definition of Done:** Nie Board/Spalte mit relevanten Tickets versehentlich löschen.

### KAN-013 · Ticket-IDs, Kerndaten und stabile Boardwechsel

- **Milestone:** M2 – Kanban-Domänenkern
- **Umsetzung:**
  1. Interne UUID und öffentliches Boardkürzel-Nummernschema implementieren.
  2. IDs auch nach Boardwechsel unverändert lassen.
  3. Kollisions-/Import-/Restorefälle testen.
- **Akzeptanz / Definition of Done:** DEV-42 bleibt bei jedem Boardwechsel dieselbe Ticketidentität.

### KAN-014 · Tickettypen, Aktivierung und Text-Custom-Fields

- **Milestone:** M2 – Kanban-Domänenkern
- **Umsetzung:**
  1. Globale optionale Typbibliothek entwickeln.
  2. Typen pro Board aktivieren, Kindticket-Erlaubnis/Sortierrang abbilden.
  3. Text-Custom-Fields strikt typunabhängig speichern.
- **Akzeptanz / Definition of Done:** Typwechsel entfernt niemals Ticket-Custom-Fields.

### KAN-015 · Globale Labels und Suchgrundlage

- **Milestone:** M2 – Kanban-Domänenkern
- **Umsetzung:**
  1. Zentrale globale Labelverwaltung erstellen.
  2. Mehrfachlabels pro Ticket abbilden.
  3. Suche und Filter unabhängig vom Board testen.
- **Akzeptanz / Definition of Done:** Label bleibt beim Boardwechsel ohne Sonderfreigabe bestehen.

### KAN-016 · Kindtickets und Elternstatus

- **Milestone:** M2 – Kanban-Domänenkern
- **Umsetzung:**
  1. Eltern-Kind-Beziehungen mit nur einer Ebene validieren.
  2. Elternstatus erledigt öffnet/schließt Kinder gemäß Beschluss.
  3. Eltern-/Kind-Boardwechsel als Transaktion gestalten.
- **Akzeptanz / Definition of Done:** Keine Unterticket-Entkopplung oder zweite Hierarchieebene.

### KAN-017 · Azyklische Abhängigkeiten und Statuskaskaden

- **Milestone:** M2 – Kanban-Domänenkern
- **Umsetzung:**
  1. Abhängigkeitsgraph persistent abbilden.
  2. Kreisbezüge verbieten und Abschlüsse blockieren.
  3. Transitiven Wiederöffnungsalgorithmus mit Backlog-Fallback implementieren.
- **Akzeptanz / Definition of Done:** Blockierte Tickets bleiben offen; Kaskade deterministisch.

### KAN-018 · WIP-Limits mit Warn- und Sperrmodus

- **Milestone:** M2 – Kanban-Domänenkern
- **Umsetzung:**
  1. WIP-Limits pro Spalte speichern.
  2. Striktes WIP vor jeder Mutation prüfen.
  3. Automatische Wiederöffnung mit Rückfall ins Backlog testen.
- **Akzeptanz / Definition of Done:** Auch API und Batch umgehen harte Limits nicht.

### KAN-019 · Boardwechsel mit Zielspalte und Typkonflikt

- **Milestone:** M2 – Kanban-Domänenkern
- **Umsetzung:**
  1. Zielspaltenwahl inkl. Backlog anbieten.
  2. Typkonflikte mit Aktivieren/Konvertieren auflösen.
  3. Untertickets und Labels unverändert mitnehmen.
- **Akzeptanz / Definition of Done:** Kein stiller Datenverlust oder impliziter Typwechsel.

### KAN-020 · Feldrevisionen, Transaktionsvalidatoren, Undo-Vorarbeit

- **Milestone:** M2 – Kanban-Domänenkern
- **Umsetzung:**
  1. Einheitlichen Mutation-Service einrichten.
  2. Basisrevision pro Entität und domänenbezogene Validierung verankern.
  3. Logische Änderungsgruppen für Undo/Sync protokollieren.
- **Akzeptanz / Definition of Done:** Web und API können keine unterschiedlichen Business-Regeln ausführen.

### KAN-021 · Responsive Navigation und Boardlayout

- **Milestone:** M3 – Responsive Weboberfläche
- **Umsetzung:**
  1. Boardtitel/-auswahl/Anzeigeauswahl und Einstellungsmenü integrieren.
  2. Spalten und Ticketkarten mobil darstellen.
  3. Touch/Tastatur und Reflow testen.
- **Akzeptanz / Definition of Done:** Auf Telefon und PC lesbares nutzbares Board.

### KAN-022 · Ticketmodal, Kontextaktionen und Markdown

- **Milestone:** M3 – Responsive Weboberfläche
- **Umsetzung:**
  1. Editierbares zentriertes Webmodal erstellen.
  2. Klon-/Lösch-/TV-anzeigen-Aktionen integrieren.
  3. Markdown sicher formatieren und Kommentare vorbereiten.
- **Akzeptanz / Definition of Done:** Öffnen im Web verändert TV nie; Webmodal kein Autoclose.

### KAN-023 · Backlog-Sidebar und Drag-and-drop

- **Milestone:** M3 – Responsive Weboberfläche
- **Umsetzung:**
  1. Backlog links über runden Button einblenden.
  2. Tickets zwischen Spalten/Backlog draggen.
  3. Tastaturalternative und WIP-Fehler anzeigen.
- **Akzeptanz / Definition of Done:** Aktionen funktionieren mit Maus, Touch und Tastatur.

### KAN-024 · Priorisierung, Suche, Filter und manuelles Ordnen

- **Milestone:** M3 – Responsive Weboberfläche
- **Umsetzung:**
  1. Konfigurierbaren Prioritätsscore anbinden.
  2. Spezifische und freie Filter-/Sortierwahl bauen.
  3. Manuellen Rang bei Sortierwechsel erhalten.
- **Akzeptanz / Definition of Done:** Aktive Sortierungen bleiben nach Neuladen und Reboot erhalten.

### KAN-025 · Einstellungen, Theme und Zweisprachigkeit

- **Milestone:** M3 – Responsive Weboberfläche
- **Umsetzung:**
  1. Settings-Kategorien und Detaildialoge implementieren.
  2. DE/EN-Ausgabe, Hell/Dunkel und Farben integrieren.
  3. Alle Einstellungswerte über API zugänglich machen.
- **Akzeptanz / Definition of Done:** Jede freigegebene Einstellung kann auch programmatisch verändert werden.

### KAN-026 · Splashscreen, Ladescreen und Undo-Toast

- **Milestone:** M3 – Responsive Weboberfläche
- **Umsetzung:**
  1. Branding-Splash ohne künstliche Pause ergänzen.
  2. Laden/Synchronisieren/Fehler-Zustände sichtbar machen.
  3. Undo-Toast nur bei aktivierter Undo-Historie und echter Änderung zeigen.
- **Akzeptanz / Definition of Done:** Kein blockierendes Dekor und kein Toast bei Undo=0.

### KAN-027 · Grafik-Prototyp auf realem Raspberry Pi 3 B+

- **Milestone:** M4 – Native TV-Laufzeit
- **Umsetzung:**
  1. SDL2-/KMS-/Fallbacks prüfen.
  2. Font-/Bild-/Textperformance messen.
  3. Ausgewählte Architektur per Benchmark dokumentieren.
- **Akzeptanz / Definition of Done:** Pi 3 B+ mit ARM32 erreicht nachweisbar nutzbare Darstellung.

### KAN-028 · Native TV-Boards und Displayformate

- **Milestone:** M4 – Native TV-Laufzeit
- **Umsetzung:**
  1. Board/Spalten/Karten nativ zeichnen.
  2. Hoch-, Quer-, Ultrawide- und kleine Formate adaptieren.
  3. Dirty-Region-Update und Speichergrenzen messen.
- **Akzeptanz / Definition of Done:** Lesbare Darstellung ohne browserpflichtigen TV-Prozess.

### KAN-029 · Ticketdetails, Markdown und Bildlinks auf TV

- **Milestone:** M4 – Native TV-Laufzeit
- **Umsetzung:**
  1. Ticketoverlay mit gedimmtem Hintergrund aufbauen.
  2. Markdown-Tabellen, Code, Kommentare, Bilder sicher anzeigen.
  3. Timeouts und Maximal-Overlayzahl umsetzen.
- **Akzeptanz / Definition of Done:** 0–300 min; bei Platzmangel nur ein Overlay.

### KAN-030 · Backlog-Overlay und Kontext-Rückkehr

- **Milestone:** M4 – Native TV-Laufzeit
- **Umsetzung:**
  1. Backlog als großen Overlaymodus integrieren.
  2. Alte Ticket-Overlays zwischenspeichern/wiederherstellen.
  3. Scrollposition bei Rückkehr aus Backlogticket erhalten.
- **Akzeptanz / Definition of Done:** Backlog-Priorität und Timer-Reset entsprechen Spezifikation.

### KAN-031 · Unterticket-Overlay und Parent-Rücksprung

- **Milestone:** M4 – Native TV-Laufzeit
- **Umsetzung:**
  1. Unterticketansicht aus Elternoverlay erstellen.
  2. Eltern-Rückkehrkontext/Timeout verwalten.
  3. Kürzlich offene Kontextkette bei fremdem Ticket verwerfen.
- **Akzeptanz / Definition of Done:** Keine falschen Rücksprünge in altes Elternoverlay.

### KAN-032 · Highlights, Auto-Scroll und 2D-Überlauf

- **Milestone:** M4 – Native TV-Laufzeit
- **Umsetzung:**
  1. Mehrfach-Highlights/Dimmen integrieren.
  2. Spalten vertikal nacheinander, danach horizontal scrollen.
  3. Manuelle Navigation/Timer/Loop testen.
- **Akzeptanz / Definition of Done:** Weder überlagerte Ansichten noch Website werden automatisch gescrollt.

### KAN-033 · Ansichtswechsel und Anzeigerotation

- **Milestone:** M4 – Native TV-Laufzeit
- **Umsetzung:**
  1. TV-Board/Gesamtübersicht/Statistik als Ansichtszustände definieren.
  2. Auswahl, Intervall und Reihenfolge speichern.
  3. Rotation bei Overlays pausieren.
- **Akzeptanz / Definition of Done:** Rotation standardmäßig aus, Neustart stellt gespeicherte Ansicht wieder her.

### KAN-034 · Animationen, Zustandsrestore und Dauerbetriebstest

- **Milestone:** M4 – Native TV-Laufzeit
- **Umsetzung:**
  1. Vier Animationsprofile implementieren.
  2. Overlay-/Scroll-/Boardzustand persistent sichern.
  3. systemd-Absturz und Langzeitbetrieb testen.
- **Akzeptanz / Definition of Done:** TV startet nach Crash zuverlässig in vorherigem Zustand.

### KAN-035 · Sichere PWA-Basis und Offline-Caches

- **Milestone:** M5 – Offline-PWA und Live-Sync
- **Umsetzung:**
  1. Service Worker für HTTPS hinzufügen.
  2. Nur besuchte Boards und benötigte Assets lokal speichern.
  3. LAN-HTTP-Einschränkung klar anzeigen.
- **Akzeptanz / Definition of Done:** Offline-PWA funktioniert mit vertrauenswürdigem HTTPS, keine globale Autocache-Pflicht.

### KAN-036 · Offline-Operationsqueue und temporäre IDs

- **Milestone:** M5 – Offline-PWA und Live-Sync
- **Umsetzung:**
  1. IndexedDB-Queue mit persistenten op_ids erzeugen.
  2. Vorläufige Ticket-IDs beim Sync mappen.
  3. Mehrtab-Koordination und Wiederholungstest implementieren.
- **Akzeptanz / Definition of Done:** Kein doppeltes Ticket bei Netzwerk-Wiederholungen.

### KAN-037 · Drei-Wege-Merge und Konflikt-UI

- **Milestone:** M5 – Offline-PWA und Live-Sync
- **Umsetzung:**
  1. Feld-/Revision-Basissnapshot führen.
  2. Unabhängige Änderungen automatisch übernehmen.
  3. Gleiche-Feld-Konflikte mit Basis/Server/Lokal anbieten.
- **Akzeptanz / Definition of Done:** Keine stillen Überschreibungen bei Konkurrenzänderungen.

### KAN-038 · Backup-Epoche und Offline-Reconciliation

- **Milestone:** M5 – Offline-PWA und Live-Sync
- **Umsetzung:**
  1. Server-Epoche bei Restore wechseln.
  2. Alte Queue gegen wiederhergestellten Zustand prüfen.
  3. Gelöschte IDs/inkompatible Operationen als Konflikte zeigen.
- **Akzeptanz / Definition of Done:** Kein ungeprüftes Wiederauferstehen alter Tickets.

### KAN-039 · Sitzungsablauf und Queue-Sicherheit

- **Milestone:** M5 – Offline-PWA und Live-Sync
- **Umsetzung:**
  1. Externe Auth-Abläufe an Offline-Sync koppeln.
  2. Bei 401 auf Reauth warten.
  3. Queue beim Verbindungsabbruch/reload erhalten.
- **Akzeptanz / Definition of Done:** Keine Auth-Umgehung, kein lokaler Datenverlust.

### KAN-040 · Echtzeit Push und Reconnect

- **Milestone:** M5 – Offline-PWA und Live-Sync
- **Umsetzung:**
  1. Server-Ereignisstrom bauen.
  2. Web und TV inkrementell aktualisieren.
  3. Nach Reconnect Server-Revisionsabgleich durchführen.
- **Akzeptanz / Definition of Done:** Ohne manuelles Reload konsistente Anzeigen.

### KAN-041 · Markdown-Kommentare und Ticketlinks

- **Milestone:** M6 – Ticketfunktionen und Automatik
- **Umsetzung:**
  1. Kommentarmodell/Migrationen aufbauen.
  2. Kommentare im Web und TV einbauen.
  3. Unabhängige Ticketlinks verwalten.
- **Akzeptanz / Definition of Done:** Kommentar und Verknüpfung über API verfügbar.

### KAN-042 · Papierkorb, Kaskadenlöschung und Recovery

- **Milestone:** M6 – Ticketfunktionen und Automatik
- **Umsetzung:**
  1. Eltern-Kind-Kaskade + Abhängigkeitsauflösung bauen.
  2. Warnvorschau erstellen.
  3. Wiederherstellung und Aufbewahrungstage testen.
- **Akzeptanz / Definition of Done:** Keine verwaisten Datensätze, Restore prüft Konflikte.

### KAN-043 · Undo/Redo-Verlauf nach beschlossenen Regeln

- **Milestone:** M6 – Ticketfunktionen und Automatik
- **Umsetzung:**
  1. Pro-Board Historientiefe 0–1000 umsetzen.
  2. Gruppierte und mehrschrittige Rollbacks integrieren.
  3. Persistenz und Undo=0 testen.
- **Akzeptanz / Definition of Done:** Undo ausschaltbar, Papierkorb davon unabhängig.

### KAN-044 · Flexible Wiederholungstickets

- **Milestone:** M6 – Ticketfunktionen und Automatik
- **Umsetzung:**
  1. Regelschema für Zeitintervalle entwerfen.
  2. Scheduler idempotent implementieren.
  3. „Erst nach Abschluss“ samt DST/Neustart prüfen.
- **Akzeptanz / Definition of Done:** Keine doppelten Wiederholungstickets.

### KAN-045 · Archivierung manuell und zeitgesteuert

- **Milestone:** M6 – Ticketfunktionen und Automatik
- **Umsetzung:**
  1. Archiv-Speicherzustand und Suche ergänzen.
  2. Massenarchivierung und optionalen Frist-Scheduler bauen.
  3. Reaktivierung/Restore testen.
- **Akzeptanz / Definition of Done:** Archivierte Tickets bleiben vollständig erhalten.

### KAN-046 · Filterbasierte Massenaktionen mit Vorschau

- **Milestone:** M6 – Ticketfunktionen und Automatik
- **Umsetzung:**
  1. Suchausdruck in Batch-Selektor übernehmen.
  2. Massenupdate atomar/mit Konfliktbehandlung ausführen.
  3. Destruktive Vorschau und Undo-Gruppierung einbauen.
- **Akzeptanz / Definition of Done:** Kein unbeabsichtigtes Löschen ohne Bestätigung.

### KAN-047 · Deadline- und WIP-Warnhinweise

- **Milestone:** M6 – Ticketfunktionen und Automatik
- **Umsetzung:**
  1. Datum und optionale Uhrzeit unterscheiden.
  2. Konfigurierbare optische Deadline-Warnung bauen.
  3. WIP-Warnungen ohne Popup implementieren.
- **Akzeptanz / Definition of Done:** Visuelle Hinweise auf TV/Web ohne Push oder TV-Unterbrechung.

### KAN-048 · Globale Übersicht für Web und TV

- **Milestone:** M7 – Statistik, Backup und Datenportabilität
- **Umsetzung:**
  1. Boardübergreifende Statuskennzahlen aggregieren.
  2. Web-Navigation zu Boards/Tickets integrieren.
  3. TV-Gesamtübersicht adaptiv rendern.
- **Akzeptanz / Definition of Done:** Kennzahlen stimmen mit SQLite und Filterkontext überein.

### KAN-049 · Historische Statistikdaten und Diagramme

- **Milestone:** M7 – Statistik, Backup und Datenportabilität
- **Umsetzung:**
  1. Ereignis-/Tagesaggregat unabhängig von Undo führen.
  2. Web-Diagramme und Zeiträume einbauen.
  3. Eigene TV-Statistikansicht mit ressourcenschonender Darstellung.
- **Akzeptanz / Definition of Done:** Statistik bleibt bei Undo-Tiefe 0 verfügbar.

### KAN-050 · Import/Export mit vollständigem JSON-Roundtrip

- **Milestone:** M7 – Statistik, Backup und Datenportabilität
- **Umsetzung:**
  1. Schema- und Versionsformat festlegen.
  2. Komplette Boardbeziehungen exportieren.
  3. CSV-Einschränkungen und Import-Vorschau hinzufügen.
- **Akzeptanz / Definition of Done:** Roundtrip bewahrt Links, Kommentare, Untertickets und Labels.

### KAN-051 · Konsistente unverschlüsselte Backups und Speicherortwechsel

- **Milestone:** M7 – Statistik, Backup und Datenportabilität
- **Umsetzung:**
  1. SQLite-Onlinebackup und Timer einrichten.
  2. Secrets sicher aus Backup herausfiltern.
  3. Datenmigration auf SSD mit Verifikation und Rollback testen.
- **Akzeptanz / Definition of Done:** Backup auf laufendem Server bleibt wiederherstellbar; keine API-Secrets.

### KAN-052 · Vollständige und selektive Backup-Wiederherstellung

- **Milestone:** M7 – Statistik, Backup und Datenportabilität
- **Umsetzung:**
  1. Preview und Bestätigung implementieren.
  2. Einzel-Board-Import mit ID-/Linkprüfung bauen.
  3. Restore-Epoche erhöhen und Queue-Konflikte testen.
- **Akzeptanz / Definition of Done:** Andere Boards bleiben beim selektiven Restore unangetastet.

### KAN-053 · Sichere LAN-/Proxy-Ingress-Trennung

- **Milestone:** M8 – Sicherheit und ChatGPT
- **Umsetzung:**
  1. LAN-HTTP-Ingress und extern geschützten Ingress sauber separieren.
  2. Reversproxy-Header-Bypass verhindern.
  3. Securitytest direkt/über Proxy ausführen.
- **Akzeptanz / Definition of Done:** Externer Zugriff kann keine LAN-Ausnahme erzwingen.

### KAN-054 · Externes gemeinsames Passwort und Sessions

- **Milestone:** M8 – Sicherheit und ChatGPT
- **Umsetzung:**
  1. Argon2id-Passcode-Speicherung aufbauen.
  2. Session-Cookies/CSRF/Rate-Limit implementieren.
  3. Logout/Timeout/Widerruf E2E testen.
- **Akzeptanz / Definition of Done:** Externe Webaktionen nur mit gültiger Sitzung.

### KAN-055 · API-Schlüsselverwaltung und Autorisierung

- **Milestone:** M8 – Sicherheit und ChatGPT
- **Umsetzung:**
  1. Getrennte widerrufbare API-Keys entwerfen.
  2. Hashing/Logging ohne Secrets einbauen.
  3. Test für Revocation und Bulk-/Restore-Rechte schreiben.
- **Akzeptanz / Definition of Done:** Weder Webpasscode noch API-Keys in Backups.

### KAN-056 · REST/OpenAPI-Vollständigkeit

- **Milestone:** M8 – Sicherheit und ChatGPT
- **Umsetzung:**
  1. Domainfunktionen versioniert abbilden.
  2. Alle Einstellungen les-/schreibbar exponieren.
  3. OpenAPI-Schema konsistent testen.
- **Akzeptanz / Definition of Done:** Web und GPT nutzen identische Business-Regeln.

### KAN-057 · MCP-Adapter und GPT-Actions-Schema

- **Milestone:** M8 – Sicherheit und ChatGPT
- **Umsetzung:**
  1. MCP-Toolliste aus OpenAPI ableiten.
  2. GPT-Actions-Schema und Tooloperationen erstellen.
  3. Ambiguität, Undo, Displaysteuerung und Listenführung testen.
- **Akzeptanz / Definition of Done:** Selbst konfigurierbare End-to-End-ChatGPT-Steuerung bereit.

### KAN-058 · Integrationsassistent und manuelle Anleitung

- **Milestone:** M8 – Sicherheit und ChatGPT
- **Umsetzung:**
  1. MCP- und GPT-Actions-Setup als getrennte Wege bauen.
  2. Verbindungstest ohne Schlüssel-Leak integrieren.
  3. DE-/EN-Schritt-für-Schritt-Doku schreiben.
- **Akzeptanz / Definition of Done:** Nutzer kann Verbindung selbständig einrichten und prüfen.

### KAN-059 · Sicherheits- und Datenschutz-Audit

- **Milestone:** M8 – Sicherheit und ChatGPT
- **Umsetzung:**
  1. XSS/SSRF/CSRF/Injection-Testmatrix ausführen.
  2. No-Telemetry/No-Remote-CDN nachweisen.
  3. Auth- und Offline-Cache-Risiken dokumentieren.
- **Akzeptanz / Definition of Done:** Bekannte kritische Sicherheitslücken geschlossen.

### KAN-060 · ChatGPT Sprachmodus und Public-Integration prüfen

- **Milestone:** M8 – Sicherheit und ChatGPT
- **Umsetzung:**
  1. Diktat-Text und Toolaufrufe im real verfügbaren Modus testen.
  2. Live-Voice-Unterstützung separat dokumentieren.
  3. Öffentliche Integration vorbereiten/Einreichvoraussetzungen prüfen.
- **Akzeptanz / Definition of Done:** Keine erfundene Live-Voice- oder Verzeichnis-Freigabe.

### KAN-061 · Manueller Updateprozess mit Backup und Rollback

- **Milestone:** M9 – Releasepakete und OS-Image
- **Umsetzung:**
  1. Update-CLI/Web/API-Aktionen integrieren.
  2. Backup vor Migration und Recovery automatisieren.
  3. Keine automatische Updateprüfung implementieren.
- **Akzeptanz / Definition of Done:** Update nur auf explizite Aktion und mit getesteter Recovery.

### KAN-062 · Raspberry-Pi-Flash-Image

- **Milestone:** M9 – Releasepakete und OS-Image
- **Umsetzung:**
  1. Pi OS 13 Imagebau definieren.
  2. Kiosk-Start, Setup-Assistent und Services integrieren.
  3. Image frisch flashen und Boot/Setup testen.
- **Akzeptanz / Definition of Done:** Erstinstallation auf realer SD-Karte funktioniert.

### KAN-063 · Drei Plattform-Releases mit Checksummen

- **Milestone:** M9 – Releasepakete und OS-Image
- **Umsetzung:**
  1. ARM32/ARM64/AMD64 Release-Binaries erzeugen.
  2. install.sh und Images paketieren.
  3. SHA256, Version, Git-Tag, Changelog zuordnen.
- **Akzeptanz / Definition of Done:** Reproduzierbare und überprüfbare Artefakte vorhanden.

### KAN-064 · GitHub Actions Dev/Beta/Stable

- **Milestone:** M9 – Releasepakete und OS-Image
- **Umsetzung:**
  1. CI-Tests und Cross-Build-Gates koppeln.
  2. Release-Workflows nach Tags erstellen.
  3. Stable an Hardware-/Security-Abnahme binden.
- **Akzeptanz / Definition of Done:** Fehlende Quality Gates verhindern Stable-Markierung.

### KAN-065 · DE-/EN-Installations- und Betriebshandbuch

- **Milestone:** M9 – Releasepakete und OS-Image
- **Umsetzung:**
  1. Setup für LAN/HTTPS/PWA dokumentieren.
  2. Kiosk/Desktop/Headless, Backup, Sicherheit beschreiben.
  3. Problembehebung Pi 3 B+ und Display hinzufügen.
- **Akzeptanz / Definition of Done:** Benutzer kann ohne Chat-Hilfe installieren und warten.

### KAN-066 · Realgerät-Benchmark und Dauerlasttest

- **Milestone:** M10 – Abnahme und Stable 1.0
- **Umsetzung:**
  1. Pi 3 B+ und je einen ARM64/AMD64-Testlauf absolvieren.
  2. TV-Scroll, mehrere Overlays, Markdown und 1000+ Tickets messen.
  3. Regressionen und Budget dokumentieren.
- **Akzeptanz / Definition of Done:** Messprotokolle auf realer Hardware vorhanden.

### KAN-067 · End-to-End-Abnahmesuite

- **Milestone:** M10 – Abnahme und Stable 1.0
- **Umsetzung:**
  1. Web-/Offline-/API-/TV-/Import- und Backup-Szenarien ausführen.
  2. Zwei-Client-Konflikt und Restore-Epoche prüfen.
  3. Kritische Regressionsfehler beheben.
- **Akzeptanz / Definition of Done:** Sämtliche vereinbarten Kernworkflows nachweisbar funktionsfähig.

### KAN-068 · Sicherheits-/Lizenz-/Releaseprüfung

- **Milestone:** M10 – Abnahme und Stable 1.0
- **Umsetzung:**
  1. Source-available-Lizenz rechtlich freigeben.
  2. Secrets und Remote-Zugriffe nochmals auditieren.
  3. Third-party-Lizenzen und Markenfreigabe dokumentieren.
- **Akzeptanz / Definition of Done:** Keine unklaren Rechte oder kritischen Leaks im Release.

### KAN-069 · Stable 1.0 veröffentlichen

- **Milestone:** M10 – Abnahme und Stable 1.0
- **Umsetzung:**
  1. Release-Gates final abhaken.
  2. Getaggte GitHub-Artefakte samt Checksummen ausliefern.
  3. Release Notes und bekannte Plattformgrenzen veröffentlichen.
- **Akzeptanz / Definition of Done:** Stable-Tag entspricht tatsächlich getesteten Artefakten.

### KAN-070 · Offene Folgearbeiten sauber dokumentieren

- **Milestone:** M10 – Abnahme und Stable 1.0
- **Umsetzung:**
  1. Nicht implementierte optionale Details und Plattformlimits inventarisieren.
  2. Tickets für Post-1.0 statt falscher Fertigmeldung anlegen.
  3. Nutzer-Handbuch und Issues aktualisieren.
- **Akzeptanz / Definition of Done:** Offene Punkte transparent, keine 100-Prozent-Behauptung ohne Belege.

## Workflow-Regeln

1. Erst das bestehende GitHub-Repository prüfen; keine neue Repositorystruktur blind überschreiben.
2. Für jede tatsächlich auf GitHub übertragene Aufgabe die KAN-Planungs-ID im Titel oder Body belassen, damit Rückverfolgbarkeit gewährleistet ist.
3. In einem echten Issue zusätzlich ausführbare Testbefehle, verantwortliche Komponente und abhängige Issues ergänzen.
4. Aufgaben nur dann schließen, wenn Implementierung, Tests, CI und Dokumentation objektiv geprüft sind.
5. Bei nicht verfügbarem Hardwaretest die Aufgabe als offen/blockiert markieren, niemals aus der Vermutung heraus abschließen.
6. Öffentliche ChatGPT-Integration und Live-Voice sind externe Verfügbarkeitsfragen; die selbstkonfigurierte API-Integration ist unabhängig davon abnehmbar.
