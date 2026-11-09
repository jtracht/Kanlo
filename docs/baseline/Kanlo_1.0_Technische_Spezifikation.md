# Kanlo 1.0 – Technische Spezifikation und Umsetzungsplan

**Stand:** 8. November 2026  
**Status:** Verbindliche Anforderungsbaseline, technischer Implementierungsentwurf; keine Aussage, dass Funktionen bereits implementiert sind.  
**Plattformen:** Debian 13 / Raspberry Pi OS 13 (armhf/ARM32, aarch64/ARM64, amd64/x86-64).  
**Primäre Zielhardware:** Raspberry Pi 3 B+ (1 GiB RAM) und neuere Raspberry Pis.  
**Produkt:** Kanlo, selbst gehostetes, TV-zentriertes Kanban-System mit Web-/PWA-Verwaltung und ChatGPT-Anbindung.

> **Geltung:** Dieses Dokument konsolidiert die im Interview beschlossenen Funktionen. Konkrete Bibliotheken sind vorgeschlagene technische Lösungen und dürfen nach belastbaren Benchmarks und Kompatibilitätstests ersetzt werden, nicht jedoch Produktentscheidungen. Offizielle ChatGPT-Kompatibilität, API-Verfügbarkeit und öffentliche Veröffentlichung müssen zum Implementierungszeitpunkt real geprüft werden.

## 1. Ziel, Betrieb und Grenzen

### 1.1 Ziel

Kanlo läuft vollständig auf **einem** Debian-/Raspberry-Pi-Gerät. Dort befinden sich Backend, SQLite-Datenbank, Webserver, lokale TV-Anzeige (falls Display angeschlossen) und Systemdienste. Smartphones/PCs öffnen lediglich die von diesem Gerät bereitgestellte Website; ChatGPT kommuniziert über eine abgesicherte API. Die TV-Anzeige ist primär passiv und wird ausschließlich über Web und ChatGPT gesteuert.

Das Produkt priorisiert niedrigen Speicherbedarf, kurze Reaktionszeiten, offline nutzbaren lokalen Server, zuverlässige Datenhaltung und gute Lesbarkeit auf verschiedensten Displays. Es ist keine allgemeine Projektmanagementsuite.

### 1.2 Nichtziele für 1.0

- Keine zusätzliche Mehrserver-Datenhaltung, Replikation oder Failover-Cluster.
- Keine drahtlose TV-Übertragung oder Bildschirmspiegelung in der Website.
- Keine eigene Spracherkennung oder Mikrofonanbindung am Servergerät.
- Keine Benutzerkonten-/Rollenverwaltung; kein verpflichtendes LAN-Login.
- Keine allgemeine Workflow-Regelengine; keine ausgehenden Webhooks; kein installierbares Drittanbieter-Plugin-System innerhalb Kanlo.
- Keine dauerhafte Synchronisierung zu GitHub Issues/Gitea/Azure, keine externe Kalendersynchronisierung.
- Keine lokale Datei-Upload-/Anlagenablage; externe Links und verlinkte Bilder sind erlaubt.
- Keine Zeiterfassung, keine Kalenderansicht, keine Swimlanes, keine Boardvorlagen/Boardduplizierung.
- Keine Push-Benachrichtigungen außerhalb geöffneter Kanlo-Oberflächen; keine Telemetrie.
- Kein eigener Bildschirmschoner, keine automatische Abdunklung, kein automatisches Display-Standby.
- Keine automatische Software-Aktualisierung und keine automatische Updateprüfung.
- Kein separates .deb-Releasepaket, kein Kanlo-APT-Repository für 1.0.

## 2. Deployment-Topologie

```text
                         ┌──────────────────────────────────┐
                         │    EIN Raspberry Pi / Debian 13  │
                         │                                  │
 HDMI/Display ◄──────────│ Kanlo TV-Prozess (native Kiosk) │
                         │         │ lokale IPC/Eventbus    │
                         │ Kanlo Backend / REST / OpenAPI  │
                         │         │ SQLite (WAL)           │
                         │ lokale Backups, Scheduler        │
                         │         │ Webserver (PWA-Dateien)│
                         └─────────┼────────────────────────┘
                                   │
                  LAN HTTP         │        Extern HTTPS (Reverse Proxy)
             Smartphone/PC ────────┤──────── Smartphone/PC
              Browser/PWA          │         Browser/PWA
              Cache + Offline-Queue│
                                   └──────── ChatGPT MCP / GPT Actions
                                             mit separatem API-Schlüssel
```

- **Kiosk-Installation:** Bei geeignetem Display automatisch minimalen grafischen Prozess starten, ohne kompletten Desktop als Pflicht. TV-Prozess nur, wenn die Hardware/Grafiksession dies erlaubt.
- **Integrationsinstallation:** Vorhandenen Debian-Desktop nicht verändern; Serverdienste im Hintergrund; lokale TV-Anzeige bei Bedarf separat starten.
- **Headless:** Ohne angeschlossenes Display bleiben Web/API/SQLite vollständig verfügbar. Später angeschlossenes Display kann TV-Prozess starten.
- **Nur ein physisches direkt angeschlossenes Display initial**; Datenmodell und Anzeigeziel-API erlauben spätere Mehrdisplay-Erweiterung. Keine Ausführung zweiter Kanlo-Serverinstanzen.
- Die sichere Unterscheidung zwischen LAN-Zugriff ohne Web-Anmeldung und externem Zugriff mit Kanlo-Zugangscode muss an **getrennten, serverseitig vertrauenswürdigen Zugangswegen** umgesetzt werden. Sie darf nicht auf einem vom Client frei gesetzten `X-Forwarded-*`-Header beruhen.

## 3. Technikvorschlag

### 3.1 Backend

**Vorschlag:** Rust mit Tokio/Axum (HTTP/OpenAPI, SSE oder WebSockets), `rusqlite`/SQLite, serde, stark typisierten Domänenoperationen; statisch gebaute Web-Assets. Vorteile: vergleichsweise geringe Laufzeit- und RAM-Kosten, ARM32/ARM64/AMD64 möglich, wenige Systemdienste. Bibliotheken und Buildkette müssen auf allen Zielarchitekturen geprüft werden.

- Ein zentraler Schreibpfad für sämtliche Mutationen (Web, API, ChatGPT, Offline-Sync, Scheduler).
- SQLite mit Foreign Keys, WAL, Busy Timeout, migrationsbasiertem Schema, konsistenten Transaktionen; keine zweite autoritative Datenbank.
- Internes Eventing zur TV-Anzeige und Push-Benachrichtigung an offene Browserclients; Events transportieren **Änderungsmetadaten**, nicht automatisch komplette Board-Snapshots.
- Serverseitige geplante Aufgaben nur für explizit beschlossene Funktionen: Backup, Papierkorb-Aufbewahrung, automatische Archivierung, Ticketwiederholung, Anzeige-/Timerzustände.
- Live-Verbindung mit automatischem Reconnect und inkrementellem State-Sync.

### 3.2 Weboberfläche / PWA

**Vorschlag:** Preact + TypeScript + Vite, lokal gebündelte Styles und Icons, keine externen Laufzeit-CDNs. Mobile-first; installierbare PWA mit Service Worker auf vertrauenswürdigem HTTPS-Ursprung. IndexedDB speichert **nur bereits besuchte/geladene Boards** und eine persistente Offline-Änderungswarteschlange.

**Wichtige technische Grenze:** Eine normale LAN-Adresse mit `http://192.168.x.x` ist in externen Browsern kein sicherer Ursprung und ermöglicht üblicherweise keine Service-Worker-PWA. Der LAN-HTTP-Aufruf bleibt für den normalen Online-Webzugriff gültig. Offline-PWA-Bearbeitung auf Smartphone/PC setzt einen vertrauenswürdigen HTTPS-Ursprung voraus, z. B. eine im LAN/extern auflösbare eigene Domain mit Reverse Proxy. `localhost`-Ausnahmen gelten nicht für den entfernten Kanlo-Server.

### 3.3 Lokales TV-Rendering

**Vorschlag:** Eigener schlanker nativer Anzeigeprozess mit SDL2 (KMS/DRM für dedizierten Kiosk, X11/Wayland als Integrationsmodus-Fallback), Textlayout über Pango/Cairo oder funktional vergleichbar. Keine dauerhafte vollständige Chromium-Instanz als zwingende TV-Runtime auf Pi 3 B+.

- Rendering als Retained Scene Graph / Dirty-Region-Updates; Event-getrieben statt permanentem 60-fps-Loop.
- Begrenzte Animations-Framerates, ressourcenadaptive Effekte und reduzierter Speicherverbrauch.
- Eingebettete Markdown-Darstellung für Ticketdetails (Absätze, Listen, Tabellen, Links, Code, Bilder) mit sicherer Auszeichnung; skalierbare Diagramme und Icons.
- Kompatibilität der Grafik-Backends, Schriften, Eingabe-/KMS-Rechte, Text- und Bilddekodierung auf ARM32/ARM64/AMD64 vor endgültiger Bibliotheksfestlegung messen.
- Ein TV-Ausfall darf Backend/Webserver/SQLite nicht abschießen; nur der TV-Prozess wird mit systemd neu gestartet.

### 3.4 Systemdienste

- `kanlo-server.service`: Backend, API, PWA, Scheduler.
- `kanlo-tv.service`: bedingt aktivierte lokale Display-Anzeige, separat restartfähig.
- `kanlo-kiosk.target` beziehungsweise angemessene Display-Start-Unit im Kiosk-Modus.
- systemd Restart bei Absturz, Startbegrenzung gegen schnelle Neustartschleifen durch systemd-Standardmechanismen; keine eigene umfangreiche Health-Monitoring-Engine.
- Systemnutzer ohne Root-Rechte, restriktive Dateirechte, explizit notwendige Display-/GPIO-/Netzwerkberechtigungen; keine unnötigen offenen Ports.

## 4. Datenmodell (logisch)

Alle Datensätze besitzen stabile interne IDs (UUID/ULID oder geeignete äquivalente Identifikatoren), Änderungszeit und bei editierbaren Entitäten eine Revision. Die konkreten Tabellen- und Feldnamen sind Vorschläge.

| Entität / Tabelle | Wesentliche Eigenschaften |
|---|---|
| `system_settings` | Sprache, lokaler Zeitzonenbezug, Anzeige-/Backup-/Netzwerkoptionen, Serverinstanz-ID, Daten-Epoche |
| `boards` | UUID, Name, dauerhaft eindeutiges Kürzel, Farb-/Darstellungseinstellungen, nächste Ticketnummer |
| `board_columns` | Zugehöriges Board, Name, Farbe, Reihenfolge, Done-Markierung, optionales WIP-Limit und Modus |
| `ticket_types` | Globale Typdefinition: Name, Farbe, Icon, Sortierrang, Kindticket-Erlaubnis; **keine typgebundenen Custom Fields** |
| `board_ticket_types` | Welche globalen Typen auf welchem Board aktiviert sind |
| `tickets` | Interne ID, **unveränderliche öffentliche ID** (`DEV-42`), Board, optional Spalte (NULL = Backlog), Parent-ID (max. eine Ebene), Titel, Markdown-Beschreibung, Typ, Priorität 0–255, Deadline/optionale Uhrzeit, Fertig-/Archiv-/Papierkorbstatus, Revision, manuelle Rangposition, letzte aktive Spalte |
| `ticket_custom_fields` | Ticket-ID, frei vergebener Name, frei vergebener **Textwert**; keine anderen Custom-Field-Datentypen |
| `labels` / `ticket_labels` | **Global** verfügbare farbige Labels und Mehrfachzuordnung pro Ticket |
| `ticket_dependencies` | Abhängiges Ticket → erforderliches Ticket, boardübergreifend, keine Zyklen |
| `ticket_links` | Sonstige explizite Verknüpfungen zwischen unabhängigen Tickets, auch boardübergreifend |
| `ticket_comments` | Ticket-ID, Markdown-Text, Zeitstempel, stabile ID; ohne Benutzerkontensystem |
| `recurrence_rules` | Wiederholungsregeln (z. B. jeden zweiten Montag), Terminlogik, optional erst nach Fertigstellung erneut erstellen |
| `saved_view_state` | Separate Sortier-/Filter-/Anzeigeoptionen für Browser- und TV-Kontext, pro Display erweiterbar |
| `tv_state` | Aktive Ansicht/Board, Overlays, Scrollpositionen, Highlights, aktive Timer/Rotation |
| `undo_history` | Logische Änderungseinheiten pro Board, konfigurierbare Tiefe 0–1000, Standard 100 |
| `trash_entries` | Lösch-/Wiederherstellungsmetadaten samt Beziehungen und Ablauf der Aufbewahrungsfrist |
| `statistics_events` / `statistics_buckets` | Für historische Diagramme benötigte Änderungsereignisse bzw. verdichtete Tages-/Wochenwerte, **unabhängig von Undo-Tiefe** |
| `api_keys` | Namen, sicher gespeicherte **Hashwerte** widerrufbarer API-Geheimnisse, Zeitstempel, Status |
| `external_web_sessions` | Gültige externe Websitzungen, widerrufbar, zeitlich begrenzt |
| `sync_operations` / `sync_receipts` | Idempotente Clientoperationen, Serverrevision und Synchronisationsergebnisse |
| `backup_settings` | Ziele, Zeitplan, Aufbewahrung, unverfängliche Einstellungen ohne Secrets |

**Kennungen:** Das Boardkürzel bestimmt den **ursprünglich ausgegebenen Ticketnamen**; `DEV-42` bleibt nach einem Wechsel auf das Board `WEB` weiterhin `DEV-42`. Die interne UUID ist unabhängig vom aktuellen Board. Vorhandene Ticketkennungen werden nie wieder für andere Tickets verwendet, auch nicht nach Löschung/Archivierung; Importe/Kopien dürfen keine Kollision erzeugen.

**Untertickets:** Nur eine Ebene. Untertickets besitzen einen binären Fertig-Status und bewegen sich nicht unabhängig zwischen Spalten/Boards; sie gehören zum selben Board wie das Hauptticket. Per Board kann ihre Darstellung entweder im Elternticket oder zusätzlich als visuelle Karten in derselben Spalte erfolgen. WIP-Zählung wird auf eigenständig verschiebbare Tickets begrenzt, damit eine alternative Unterticketdarstellung das Limit nicht verändert.

## 5. Verbindliche Domänenregeln

### 5.1 Tickettypen, Custom Fields und Labels

- Tickettypen sind optional; ohne Typdefinition sind alle Tickets gleichartig. Globale Typbibliothek, Aktivierung je Board, Typfarb-/Icon-/Priorisierungsrangfolge und Erlaubnis für Untertickets.
- Beim Wechsel eines Tickets auf ein Board, das dessen Typ nicht aktiviert hat, **bewusste Auswahl**: Typ auf Zielboard aktivieren **oder** Ticket auf erlaubten Typ umstellen. Keine stille Konvertierung.
- **Jedes Ticket kann beliebige eigene Text-Custom-Fields tragen**, völlig unabhängig von seinem Typ. Typwechsel verändert sie nicht.
- Labels sind ausschließlich **global**; immer auf allen Boards verfügbar. Beim Boardwechsel unverändert beibehalten.
- Löschen eines Tickettyps verboten, solange irgendwo mindestens ein Ticket ihn verwendet.

### 5.2 Spalten, Status und WIP

- Beliebige boardeigene Spalten und Farben; eine oder mehrere als Done markierbar. Eintritt in Done ⇒ Ticket erledigt; Verlassen ⇒ wieder geöffnet.
- Jede Spalte kann optionales WIP-Limit tragen, Modus `warning` oder `strict`. Im strikten Modus sind normale API-/Web-/Sammelverschiebungen über das Limit gesperrt.
- Backlog ist außerhalb normaler Spalten verfügbar und separat anzeigbar.
- Board/Spalte darf nur gelöscht werden, wenn **vollständig leer** (inkl. gefilterter, versteckter und Backlog-Zugehörigkeiten; gelöschte/archivierte Daten sinnvoll prüfen).
- Beim Boardwechsel wählt Nutzer Zielspalte/Backlog; gleichnamige Spalten dürfen vorgeschlagen werden.
- Eltern und alle Untertickets wechseln **immer gemeinsam** das Board; keine Trennung.

### 5.3 Untertickets und Abhängigkeiten

- Eltern-Ticket Done ⇒ sämtliche Untertickets werden automatisch Done (Atomarität; blockierende Abhängigkeiten müssen vorher validiert werden, kein unbemerkter Regelbruch).
- Eltern-Ticket wieder öffnen ⇒ Untertickets bleiben Done, außer eine **eigene Abhängigkeit** erfordert deren Wiederöffnung.
- Tickets können andere Tickets boardübergreifend als verpflichtende Voraussetzungen haben; Zyklen verhindern.
- Abschluss blockiert, solange mindestens eine Voraussetzung nicht erledigt ist; dieselbe Regel gilt für alle Schreibwege einschließlich Offline-Abgleich und Massentransaktionen.
- Wird eine erledigte Voraussetzung wieder geöffnet, werden **alle transitiv davon abhängigen, bereits erledigten Tickets automatisch wieder geöffnet**.
- Ein dadurch wieder geöffnetes Ticket wandert in seine letzte nicht-Done-Spalte; wenn nicht existent oder wegen strikten WIP-Limits voll: **Backlog**, mit Hinweis.
- Die systembedingte Kaskade ist eine nachvollziehbare logisch gruppierte Änderung, auch für Undo.

### 5.4 Löschen und Wiederherstellen

- Beim Löschen eines Elterntickets gehen alle seine Untertickets in den Papierkorb. Abhängigkeiten anderer Tickets werden gelöst; vorher Warnung mit Folgen. Keine individuelle Auswahl je Beziehung.
- Papierkorb unterstützt Wiederherstellung und konfigurierbare automatische Löschung nach Tagen, auch wenn Undo-Tiefe 0 ist.
- Wiederherstellung aus Papierkorb berücksichtigt mögliche zwischenzeitliche ID-/WIP-/Abhängigkeits-/Board-Konflikte und lässt keine Inkonsistenz zu; nötigenfalls kontrollierte Auflösung anbieten.
- Normale Undo-Historie je Board (0–1000 Schritte, Default 100) wird nach Neustarts bewahrt. Bei 0 ist Undo samt Toast und ChatGPT-Undo deaktiviert. Web zeigt nach tatsächlichen Änderungen kurzen Undo-Toast; ChatGPT kann letzte oder bis zu referenzierter früherer Aktion rückgängig machen; große Rücknahmen bestätigen.

### 5.5 Sortieren, Filtern, Priorisieren, Sammelaktionen

- Ticket-Importanz 0–255; errechneter Gesamtscore kombiniert konfigurierbar Typ-Rang, manuelle Wichtigkeit und Deadline-Dringlichkeit. Filter/Sortierung nach Eigenschaften, Text, Labels, Typ, Datum, Priorität, Status und benutzerdefinierten Textfeldern.
- Sortierung auch manuell pro Spalte: Drag-and-drop und ChatGPT können Reihenfolge dauerhaft setzen; Umschalten auf andere Sortierung überschreibt den manuellen Rang **nicht**.
- Web- und TV-Sortierung/Filter sind unabhängig; für spätere mehrere Displays ebenfalls separat; Zustände nach Neustart persistent.
- Gezielte Mehrfachauswahl und **filterbasierte Massenoperationen** (Verschieben, Bearbeiten, Prioritäten, Archivierung, Löschen), mit Voransicht und Bestätigung bei umfangreichen/destruktiven Aktionen. Konflikt-, Abhängigkeits- und WIP-Regeln bleiben in Kraft.

### 5.6 Wiederholung, Archiv, Deadlines

- Wiederkehrende Tickets: täglich/wöchentlich/monatlich/jährlich oder flexible Intervalle (z. B. 2. Montag, alle drei Wochen), optional erst nach Abschluss des Vorgängers; lokal zeitgesteuert, ohne allgemeine Workflow-Engine.
- Abgeschlossene Tickets manuell einzeln/gebündelt archivieren; optionale automatische Archivierung nach konfigurierbarer Frist (standardmäßig aus). Archivierte Tickets vollständig such- und wiederherstellbar.
- Deadlines: Datum, optional Uhrzeit; Datum allein gilt bis Ende des lokalen Tages. Dezente visuelle Warnungen bei baldiger Fälligkeit/Überfälligkeit auf TV und Web; konfigurierbarer Warnhorizont; keine eigenständig störenden TV-Popups.
- Keine Push-Benachrichtigung bei geschlossener Website.

## 6. TV-Oberfläche und Interaktionsverhalten

### 6.1 Ansichten und Bedienung

- Lokale, für TV optimierte Vollbild-Kanban-Ansicht: Boardtitel, Spalten, Ticketkarten; Karten primär Titel, optionaler Typ, Priorität 0–255 und Deadline.
- Zusätzlich: **boardübergreifende Gesamtübersicht** (z. B. offen, erledigt, überfällig, blockiert) und **separate historische Statistikansicht** mit TV-tauglichen Diagrammen.
- Optionaler automatischer Wechsel durch frei gewählte Boards, Gesamtübersicht und Statistikansicht, mit konfigurierbarer Reihenfolge/Intervall; **standardmäßig aus**.
- TV ist **Anzeige**, nicht primäre Bearbeitungsoberfläche; Bedienung durch Website/ChatGPT. Kein eigener Remote-Screenstream oder Screenshot in der Website.
- Quer-/Hochformat, Ultrawide, sehr kleine und ungewöhnliche Formate adaptiv; keine Swimlanes.
- System-Splashscreen und gesonderter Ladescreen; keine künstliche Startverzögerung; Ladezustände verständlich anzeigen.
- Kein Kanlo-Bildschirmschoner, keine automatische Abdunklung oder Standby; Display-eigene Automatik liegt außerhalb Softwaregarantie.

### 6.2 Overlay-Regeln

- Ticketdetails per explizitem Web-„TV anzeigen“ oder ChatGPT zentriert mit animiertem, gedimmtem Hintergrund.
- Bis zu **zwei** Ticket-Overlays gleichzeitig, **nur bei ausreichend lesbarem Platz**; sonst eines. Drittes ersetzt ältestes. Timeout individuell 0–300 min, 0 = bleibt bis Schließen; nach TV-Neustart aktuellen Overlayzustand wiederherstellen und Timer neu starten.
- Backlog-Overlay belegt größeren Bereich, ersetzt temporär Ticket-Overlays; beim Schließen alte Overlays mit neu gestarteten Timern wieder anzeigen.
- Öffnen eines Tickets **aus dem gerade sichtbaren Backlog**: Ticket ersetzt Backlog; nach Schließen zurück zur selben Backlog-Scrollposition und Backlog-Timer neu starten. Direkter Ticketaufruf von außerhalb schaltet zum zugehörigen Board.
- Unterticket aus Eltern-Overlay öffnet statt des Eltern-Overlays; beim Schließen/Timeout Rückkehr zum Eltern-Overlay und erneuter vollständiger Timer, sofern Rückkehrkontext gültig; andere unabhängige Tickets unterbrechen diesen Rückkehrpfad.
- Web-Ticketmodal ist editierbar, **nie** automatisch schließen; Öffnen eines Webmodals darf nichts auf dem TV verändern.

### 6.3 Scrolling, Highlights und Animationen

- Auto-Scroll **nur auf dem TV**, optional, konfigurierbare Intervalle und Pausen; nach manuellem/ChatGPT-Scroll Timer neu beginnen; am Ende zum Anfang.
- In 2D-Überlauf **erst vertikal, dann horizontal**: sichtbare Spalten **nacheinander jeweils vollständig vertikal** durchscrollen, dann zur nächsten horizontalen Spaltengruppe; nur aktive Ansicht/Overlay scrollen.
- Sprach-/API-Befehl „Zeige Ticket im Board“ springt in Board und scrollt/highlightet Karte **ohne** Ticketoverlay; „Öffne Ticket“ zeigt Overlay.
- Mehrere Tickets dürfen gleichzeitig hervorgehoben werden; sonstige Elemente abdimmen; Standarddauer ~20–30 s, konfigurierbar oder dauerhaft.
- Animationsprofile `Aus`, `Dezent`, `Flüssig` (Standard), `Dynamisch`; adaptiv und über Web/ChatGPT steuerbar.
- Echtzeitaktualisierung bei API-/Web-/Scheduleränderungen per internem Eventkanal; grafische Oberfläche nur betroffene Regionen neu zeichnen.

## 7. Weboberfläche und PWA

- Mobilfähig, Bedienung durch Maus, Touch und vollständig durch Tastatur; ausreichend Kontrast, lesbare Schriftgrößen, verständliche Fehlermeldungen; keine verbindliche vollständige WCAG-2.2-AA-Zertifizierung.
- Standardansicht Board, oberhalb Boardauswahl und zukünftige Displayauswahl, Einstellungen rechts; linker Backlog-Button/Sidebar.
- Linksklick/Touch auf Ticket öffnet **editierbares zentriertes Webmodal** ohne TV-Auswirkung. Kontextmenü/Mehr-Menü mit Löschen, Klonen, TV-Anzeige, Details.
- Drag-and-drop von Tickets zwischen Spalten und bei manueller Sortierung innerhalb der Spalten; barrierearme Tastaturalternative.
- Tickets mit Markdown-Beschreibung (Überschriften, Listen, Checklisten, Tabellen, Code, Links, Bilder aus externen Quellen); separate zeitgestempelte Markdown-Kommentare.
- Deutsche/englische Oberfläche, Einrichtung und Dokumentation; spätere i18n erweiterbar.
- Eigene lokale PWA-Splash-/Ladezustände; klare Offline-, Sync-, Konflikt-, Auth- und Fehlermeldungen.
- Zeigt **keine TV-Bildvorschau**, obwohl TV-Zustand und -Befehle verwaltbar sind.

## 8. Offline-Synchronisation – verbindliches Protokoll

### 8.1 Prinzip

**Einzige autoritative Datenbank:** SQLite auf dem Kanlo-Server. Der PWA-Client speichert lediglich bereits besuchte Boards mit ihren geladenen Tickets, Kommentaren, Labels/Typdefinitionen soweit benötigt, plus eine persistent gespeicherte Operationsqueue. Nicht besuchte Boards sind nicht offline verfügbar.

### 8.2 Operationen

Jede Offline-Mutation enthält mindestens:

```json
{
  "op_id": "globally-unique-operation-id",
  "client_id": "persistent-browser-profile-id",
  "server_instance_id": "kanlo-instance-id",
  "base_epoch": "restoration-or-import-generation",
  "entity_id": "stable-uuid-or-temporary-client-id",
  "base_revision": 17,
  "operation": "ticket.patch",
  "base_fields": {"importance": 100},
  "changed_fields": {"importance": 200},
  "created_at": "ISO-8601"
}
```

- `op_id` garantiert bei erneuter Zustellung **idempotente** Verarbeitung; Server speichert Receipts.
- Bei Offline-Ticketerstellung verwendet der Client temporäre IDs. Der Server weist bei erfolgreicher Übernahme endgültige UUID und öffentliche Boardkürzel-Nummer zu und meldet die Zuordnung zurück. Keine lokal endgültig reservierten Ticketnummern.
- Offline-Operationen werden in IndexedDB sicher vor dem Anzeigen als lokal gespeichert committet; Synchronisation bestätigt/entfernt nur erfolgreich quittierte Operationen.
- Mehrere Tabs dürfen nicht gleichzeitig konkurrierende Queues blind senden; Tab-Koordination über Browser BroadcastChannel/Locks oder äquivalent.

### 8.3 Smart Merge

1. Bei Verbindung zuerst Serverinstanz/Epoche, Auth-Status, aktuelle Revisionen und Idempotenz-Quittungen abgleichen.
2. Vergleich auf **Feld-/Operations-Ebene** anhand Base-Snapshot und aktueller Serverwerte; unabhängige Änderungen automatisch zusammenführen.
3. Append-only Kommentare können meist unabhängig eingefügt werden; eindeutige Zuordnungen beachten.
4. Gleichzeitige Änderungen an **demselben Feld** mit unterschiedlichen Werten oder strukturelle Konflikte (gelöscht/verschoben, Typwechsel, Zielspalte, Archivierung, Abhängigkeiten) dürfen nicht blind überschrieben werden.
5. Domainregeln (WIP, Blocker, Typaktivierung, Eltern-Kind, IDs) **auf dem aktuellen Serverzustand** erneut validieren.
6. Erfolgreiche Änderungen transaktional schreiben, neue Revision liefern und live an TV/Browser verteilen.
7. Bei echten Konflikten die Beteiligten sichtbar machen: Basisversion, lokale geplante Änderung, Serverstand. Nutzer kann lokale oder Serverversion wählen oder kontrolliert anpassen. Bis dahin bleibt die Operation in der Queue.
8. Für Änderungen über **vollständige/selektive Backup-Wiederherstellung** die neue Server-Datenepoche erkennen. Nur eindeutig sichere, referenziell valide Operationen übernehmen; konflikthafte Operationen zurückhalten und erklären. Gelöschte Daten dürfen nicht stillschweigend „wiederauferstehen“.
9. Bei abgelaufener externer Websitzung automatische Synchronisierung versuchen, aber bei 401/403 **stoppen und bis zur erneuten Anmeldung warten**. Queue bleibt unverändert erhalten.

### 8.4 Reihenfolge und Atomarität

- Abhängige Offline-Operationen (z. B. Ticket lokal erstellen, anschließend Kommentar hinzufügen) in kausaler Reihenfolge abarbeiten.
- Große Sammeloperationen erhalten einen gemeinsamen Transaktions-/Batchbezug. Vorhandene Vorschau und Bestätigung dürfen nicht durch einen Offline-Pfad umgangen werden.
- Eine Konfliktauflösung wird als neue explizite Operation mit aktueller Basisrevision gespeichert und auditiert; ein Pauschal-„last write wins“ ist verboten.
- Datenschutz: Browsercache ist nicht eigens durch Kanlo verschlüsselt/mit PIN geschützt; Gerätesperre und Browserprofil schützen ihn. Er enthält keine API-Schlüssel; lokale Daten dürfen aus den Kanlo-Einstellungen entfernt werden. Vor Löschen von **noch nicht synchronisierten Änderungen** stets warnen.

## 9. Suche, ChatGPT und API

### 9.1 Einheitliche API

Die REST/OpenAPI-Schicht bildet alle les- und schreibbaren Einstellungen und Kernaktionen ab: Boards/Spalten/Typen/Labels, Tickets/Untertickets/Kommentare/Links/Abhängigkeiten, WIP, Filter/Sortierungen, Anzeigensteuerung, Backlog/Overlays/Highlights/Scroll/Rotation, Termine/Wiederholung/Archiv/Statistiken, Undo/Papierkorb, Im-/Exporte, Backups/Restore, Netzwerk-/Integrations-/Update-Befehle soweit sicher implementiert.

- **Keine zwei unterschiedlichen Geschäftslogiken:** Web, TV-Steuerung, MCP und GPT Actions verwenden denselben Server-Kern.
- GET für Abfragen, idempotente sichere Mutationsmechanismen, strukturierte Fehlercodes (`CONFLICT`, `WIP_LIMIT`, `BLOCKED_BY_DEPENDENCY`, `AUTH_EXPIRED`, etc.).
- OpenAPI-Schema maschinenlesbar, konsistent dokumentiert, versionierte Pfade (`/api/v1/...`).
- Lange oder destruktive Operationen haben Vorschau → explizite Bestätigung → Ausführung.
- Separate, rotierbare/widerrufbare API-Schlüssel; Integrationsschlüssel niemals als Klartext in Backups.

### 9.2 ChatGPT-Anbindung

- Geführter Web-Assistent plus manuelle Einrichtung für MCP und GPT Actions; beide einzeln testbar.
- Vollständige selbst konfigurierbare Integration unabhängig von öffentlicher Veröffentlichung.
- Öffentliche Kanlo-Integration entwickeln und **nur soweit verfügbar und genehmigungsfähig** veröffentlichen/einreichen.
- Sprachbefehle über ChatGPT-App/-Eingabe; keine Spracherkennung auf Kanlo-Gerät. Die Fähigkeit, **aus einem konkreten Live-Sprachmodus** Integrationen aufzurufen, muss separat real getestet werden. Transkribierte Texteingabe und Live-Voice-Toolaufrufe nicht gleichsetzen.
- Suche: explizite IDs haben Vorrang; sonst sichtbare Tickets vor unsichtbaren desselben Boards, dann andere Boards; Mehrdeutigkeiten klar auflösen, TV-Mini-Ticketkarten nach Boards gruppieren; nach Öffnen Kontext für Folgekommandos halten.
- Einstellungen im Dialog stufenweise vorlesen: „Welche Einstellungen?“ nur Kategorien; Kategorie → Namen, Werte erst auf explizite Nachfrage; umfangreiche Listen in Fünferblöcken mit `weiter`, `stopp`, `von vorne`.
- Keine Annahme, dass ein ChatGPT-Connector ohne erreichbare, sicher konfigurierte Instanz automatisch funktioniert.

## 10. Sicherheit und Datenschutz

- **LAN-Web:** per Beschluss ohne Anmeldung auf HTTP; der Betreiber muss dem lokalen Netzwerk vertrauen. Offen kommunizieren, dass LAN-Teilnehmer schreiben/löschen können; kein vermeintliches Sicherheitsversprechen.
- **Externes Web:** HTTPS wird durch Betreiber-Reverse-Proxy terminiert; Kanlo prüft den gemeinsamen externen Zugangscode selbst und verwaltet Sessions. Für externen und LAN-Zugang **serverseitig getrennte Ingress-/Listener-Regeln**; untrusted `X-Forwarded-*` niemals als Auth-Bypass verwenden. Reverse Proxy muss ausschließlich zum geschützten Ingress routen. Externen Ingress aus dem LAN nicht versehentlich ohne Schutz öffnen.
- **Externes Web-Passwort:** starker Argon2id-Hash, Rate Limits, Sessions zeitlich begrenzt und widerrufbar; Cookies Secure/HttpOnly/SameSite, CSRF-Schutz für cookie-basierte Mutationen, origin-/hostbasierte Validierung.
- **API:** separater Schlüssel, nur gehashte Secrets im Speicher, konstantzeitnahe Prüfung, Rate Limits, sichere Rotation, auditierbare Erfolgs-/Fehlversuche ohne Preisgabe von Geheimnissen.
- **Markdown:** HTML-/Script-Sanitization, strikte CSP und URL-Prüfung; keine Ausführung eingebetteten Codes. Externe Bild-/Linkressourcen korrekt und datenschutzbewusst laden; serverseitiges Abrufen externer URLs nur mit SSRF-/Redirect-/Privatnetzschutz.
- **Import/Export:** Größen-/Tiefe-Limits, CSV-Formula-Injection-Entschärfung beim Export, JSON-Schema-Versionierung, fehlertolerante aber transaktionale Importe, eindeutige IDs prüfen.
- **Keine Telemetrie:** keine automatische Datenübertragung an Entwickler, keine Trackingskripte oder Remote-CDNs, Diagnose-Logs lokal und manuell exportierbar.
- **Secrets:** ausgeschlossen aus unverschlüsselten Backups, JSON-Boardexporten, Crashlogs und PWA-Offlinecache; nach Restore neue API-Schlüssel/Integrationsverbindung.

## 11. Backups, Import und Update

### 11.1 Backups

- SQLite-konsistente Snapshots über Backup-API oder gleichwertigen WAL-sicheren Mechanismus, nicht unkontrollierte Rohdateikopie bei aktiver DB.
- Manuell und zeitgesteuert; Speicherort für DB/Einstellungen/Backups anfangs Systemlaufwerk, später per Webassistent sicher migrierbar (inkl. externe SSD); vor Migration Sicherung und verifizierte Übernahme.
- Backups **unverschlüsselt**, beinhalten alle fachlichen Daten, nicht aber API-Schlüssel, externe Sitzungstokens oder sonstige Geheimnisse.
- Vollständige Wiederherstellung und selektive Wiederherstellung einzelner Boards mit Vorschau/Bestätigung, ID- und Linkprüfung. Vor riskantem Restore aktuelle Daten separat sichern, transaktional ersetzen.
- Backup-Restore erhöht eine Server-Datenepoche, sodass PWA-Clients einen potenziell älteren Datenstand erkennen und ihren Queue-Abgleich sorgfältig durchführen.

### 11.2 Import/Export

- Vollständiger, versionierter JSON-Boardexport inklusive Spalten, Tickets, Untertickets, Kommentaren, Text-Custom-Fields, Labels, Ticketlinks, Abhängigkeiten und benötigter Typdefinitionen, ohne Undo-Historie.
- CSV-Export tabellarisch passender Felder; komplexe Beziehungen transparent begrenzen. CSV-/JSON-Import mit Vorschau, Erkennung eindeutiger IDs, Auswahl aktualisieren/überspringen/neu; Standard niemals ungefragt überschreiben.

### 11.3 Updates/Release

- GitHub Releases enthalten `install.sh`, passende gebaute Programmartefakte für ARM32, ARM64, AMD64 sowie flashbare unterstützte Raspberry-Pi-Images; **keine separaten .deb-Pakete/kein APT-Repository**.
- Manuelles Update per SSH/Skript oder ausdrücklich angeforderter Web-/ChatGPT-Aktion. Keine automatische Prüfung oder Installation.
- Vor Update Backup, Kompatibilitätsprüfung und migrationssicherer Rollbackplan; versionierte Schema-Migrationen. Rollback bei DB-Migration muss aktiv getestet sein, nicht nur Binärdatei zurückkopieren.
- GitHub Actions: Lint, Format, Unit/Integration/Security, Cross-Build, Packaging, Artefakt-Checksummen, versionierte Dev-/Beta-/Stable-Releases. Reale Pi-3-B+-Hardwaretests vor Stable obligatorisch.

## 12. UI-Einstellungen, Statistiken und Verlauf

- Themen: Hell/Dunkel und anpassbare Farben, Hintergründe/Layout, Animationseinstellungen; alle Einstellungen über Web und ChatGPT les-/änderbar.
- Statistiken: neue/erledigte Tickets, Backlog-Verlauf, blockierte Tickets, auswählbare Zeiträume, pro Board und global; Diagramme sowohl im Web als auch eigene TV-Statistikansicht. Keine umfassende Cycle-Time-/Lead-Time-/CFD-Analytics als Pflicht.
- Optimierte 2D-TV-Darstellung; Speichern von Scroll-/Filterzustand und TV-Overlays nach Reboot, Timer auf Neustart korrekt zurücksetzen.
- Notizen/Kommentare Markdown mit Zeitstempel; keine Benutzernamen wegen fehlender Accounts erforderlich.
- Splashscreen beim Start, Ladescreen bei Initialisierung und Synchronisierung; keine überlangen Warteanimationen.

## 13. Architektur- und Risikoentscheidungen für die Implementierung

| Risiko | Gegenmaßnahme / Test |
|---|---|
| Pi 3 B+ mit ARM32 und sehr begrenztem RAM | Früh echte Messungen, native TV-Renderpipeline, keine Chromium-Pflicht für TV, inkrementelle Datenupdates |
| DRM/KMS auf Raspberry Pi OS 13 / Debian 13 | Früh auf realen Geräten Grafik-, Input-, Fonts- und Auflösungs-Tests; Fallback auf Desktop-Session im Integrationsmodus |
| HTTP-LAN vs HTTPS-PWA | LAN-HTTP ausdrücklich erhalten; Offline-PWA nur auf vertrauenswürdigem HTTPS-Ursprung garantieren, klare Ersteinrichtungsanzeige |
| Externes Login versehentlich umgehbar | Getrennte Ingress-/Auth-Pfade, keine Forwarded-Header-Autorisierung, Security-Regressionstest aus externem Netz |
| Offline-Sync nach DB-Restore | Epochen/Revisionen, Feld-3-Wege-Merge, operationale Idempotenz, Tombstones, Konflikt-UI und Integrationstests |
| WIP/Abhängigkeit/Parent-Regeln widersprechen sich | Ein Transaktionsvalidator vor jeder Aktion, auch durch Scheduler, API, Batch und Offline-Queue |
| Native TV-Markdown-/Bildanzeige zu teuer | Bilder/Fonts deckeln, Lazy-Rendering, Caches begrenzen, Markdown sicher vorparsen |
| ChatGPT Live-Voice nutzt Integration evtl. nicht | Text-/Diktat-/Live-Modus separat verifizieren, keine nicht unterstützten Veröffentlichungszusagen |
| Source-available-Lizenz legal unklar | Lizenztext und Markenrechte rechtlich prüfen; nicht als OSI-Open-Source bezeichnen |
| Wiederholung nach Server-Neustart | Idempotente Terminläufe mit eindeutiger Wiederholungsinstanz-ID, kein doppeltes Erzeugen |

## 14. Nichtfunktionale Abnahmekriterien

Die exakten Schwellen werden im frühen Pi-3-B+-Benchmark als dokumentierte Releasebudgets festgelegt, statt unbegründete Zahlen vorzutäuschen. Dennoch gelten verbindliche qualitative Gates:

1. ARM32, ARM64 und AMD64 starten reproduzierbar unter den festgelegten OS-Versionen.
2. Webserver und SQLite laufen unabhängig von angeschlossenem Display.
3. Kiosk-Modus übernimmt Anzeige nur auf ausdrückliche Auswahl; Integrationsmodus verändert vorhandenen Desktop nicht.
4. Langzeittest: TV läuft stabil; systemd startet abstürzende Prozesse neu und zuletzt gespeicherte Ansicht wird restauriert.
5. Reale Pi-3-B+-Tests messen RAM, CPU, Eingabe-/API-Latenz, Scroll/Animation, viele Tickets, lange Kommentare, Diagramme und Backups.
6. Im LAN-HTTP ist Web online benutzbar; PWA-Offline funktioniert auf vertrauenswürdigem HTTPS.
7. Zwei Browserclients und ChatGPT erzeugen Konflikte, die ohne Datenverlust korrekt aufgelöst werden.
8. Nach vollständigem/selektivem Backup-Restore werden alte Offline-Queues weder ungeprüft übernommen noch gelöscht.
9. Strikte WIP- und Abhängigkeitsregeln halten auch bei parallelen, Offline- und Massenaktionen.
10. Sämtliche beschlossenen Kanlo-Einstellungen und Funktionen sind im Web und in der selbst konfigurierten ChatGPT-Anbindung erreichbar; externe Live-Voice-Unterstützung wird ehrlich dokumentiert.
11. Kein unerwarteter Netzverkehr an Entwickler/Trackingdienste; keine Klartext-Secrets in Backups/Logs/Exports.
12. Deutsches und englisches UI, Einrichtung, APIs und Dokumentation funktionsfähig.

## 15. Entwicklungsmeilensteine (Reihenfolge)

| Meilenstein | Schwerpunkt | Abnahme-Gate |
|---|---|---|
| **M0 – Repository & Qualitätsbasis** | GitHub-Repo, Lizenzhinweis, CI, Releasestruktur, Test- und Dokumentationsregeln | CI grün, nachvollziehbare Roadmap, ARM-Buildmatrix definiert |
| **M1 – Runtime & Installer** | Rust-Dienst, SQLite, Config, systemd, Kiosk/Integration/Headless, `install.sh` | Alle unterstützten Architekturen starten ohne externe Cloud |
| **M2 – Kanban-Domänenkern** | Boards, Spalten, Tickets, Typen, Labels, Custom Fields, WIP, Untertickets, Abhängigkeiten | Transaktionale Integrationstests für widersprüchliche Regeln |
| **M3 – Web-Grundoberfläche** | Mobile UI, Modals, Backlog, Drag-and-drop, Suche/Filter/Sortierung, Einstellungen | Browser-Tests inkl. Tastatur und DE/EN |
| **M4 – Native TV-Runtime** | Board, Overlays, Scroll, Highlights, Animationen, Displays, Boot-Restore | Pi-3-B+-Messungen und physische Displaytests |
| **M5 – Offline-PWA & Live** | HTTPS-PWA, lokale Boardcaches, Offline-Queue, Merge/Conflict, Echtzeit-Push | Mehrclient-, Wiederverbindungs- und Restore-Konflikttests |
| **M6 – Erweiterte Tickets & Daten** | Kommentare, Markdown, Wiederholung, Archiv, Papierkorb, Undo, Massentools | Regel-/Recovery-/Migrationstests |
| **M7 – Views, Analytics & Datenportabilität** | Gesamtübersicht, TV-Statistik, Rotation, Statistikverlauf, Import/Export/Backups | Wiederherstellung inkl. selektivem Board, Charts auf Pi |
| **M8 – Sicherheit & ChatGPT** | LAN-/Proxy-Auth-Trennung, API-Keys, OpenAPI, MCP, GPT Actions, Setupassistent | Sämtliche Kernaktionen über ChatGPT nutzbar, Penetrationstests |
| **M9 – Release & Image** | ARM32/ARM64/AMD64 Builds, flashbares Pi-Image, Updater/Rollback, Dokumentation | End-to-End Fresh-Flash + Upgrade + Restore bestanden |
| **M10 – Stable 1.0** | Bugfixes, Security-Audit, Last-/Langzeittest, zweisprachige Docs, Release-Gates | Öffentliche Stable-Artefakte, bekannte Plattformgrenzen dokumentiert |

**Umsetzungsgrundsatz:** Erst die tragfähige Architektur und ausführbare Kernfunktionen herstellen, dann Funktionen inkrementell über Tests absichern. Keine „100 % fertig“-Behauptung ohne echte Tests und passende Releaseartefakte. Die später separat bereitgestellte öffentliche ChatGPT-Integration kann eine Fremdplattformfreigabe erfordern und darf eine bereits funktionsfähige selbst eingerichtete Integration nicht blockieren.

## 16. Lizenz- und Veröffentlichungshinweise

- Source-available, **nicht** OSI-Open-Source: privat/nichtkommerziell kostenlos gemäß Bedingungen; kommerzielle Nutzung nur mit gesonderter Erlaubnis; keine eigenständige Verteilung modifizierter Versionen; Beiträge über PRs zum Originalprojekt möglich.
- **Keine technische Lizenzaktivierung**, keine Lizenzschlüssel, keine Online-Lizenzprüfung.
- Konkreter juristischer Lizenztext, Namens-/Markenprüfung und zulässige Lizenzkombinationen sämtlicher Drittkomponenten vor Public Release durch geeignete Prüfung festlegen.
- GitHub-Repos, Issues, Milestones, Actions und Releases sind geplant; dieses Dokument behauptet nicht, dass sie bereits angelegt wurden.

---

**Nächster Arbeitsgegenstand:** Die nachfolgenden Issue-Backlog- und Codex-Masterprompt-Dateien operationalisieren diese Spezifikation. Technische Detailentscheidungen sollen nicht erneut als langes Interview an den Nutzer zurückgespielt werden, solange sie keine festgelegte Grundsatzentscheidung widersprechen.
