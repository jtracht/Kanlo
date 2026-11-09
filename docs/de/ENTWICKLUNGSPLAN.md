# Entwicklungsplan

Baseline: 70 Aufgaben, M0–M10. Originalchecklisten und Akzeptanzkriterien unter
`docs/baseline/Kanlo_1.0_GitHub_Issues.md`; ausführbare Planung je Aufgabe unter
`docs/planning/issues.json`. Planungsnummern sind keine Remote-Issue-Nummern.

| Reihenfolge | Arbeitspaket | Gate / Abhängigkeiten |
|---|---|---|
| M0 | Struktur, Lizenzstatus, Architektur, CI, Tests | KAN-001–005; CI nach Push separat prüfen |
| M1a | Rust-Start, SQLite, erster Online-Client | KAN-006–007 teilweise; lokaler Runtime-Test |
| M2a | Boards/Spalten/Tickets, permanente IDs, WIP/Revisionen | KAN-012/013/018/020 teilweise; DB-/HTTP-Tests |
| M1b | Dienste, Installer, Headless, Einrichtung | KAN-008–011; Debian und Display/Netzwerkzugang |
| M2b | Typen/Labels, Untertickets, Abhängigkeiten, Boardwechsel | KAN-012–020 vollständig; atomare Regelsuite |
| M3 | Preact/TS, DE/EN, mobile UI, Modal, Backlog, Sortieren | KAN-021–026; Browser/Touch/Tastatur |
| M4 | Native TV-Renderpipeline, Overlays, 2D-Scroll | KAN-027–034; Pi-3-B+-Hardware, kein Browserersatz |
| M5 | HTTPS-PWA, idempotente Queue, 3-Wege-Merge, Live | KAN-035–040; Auth- und Restore-Protokoll vorher fixieren |
| M6 | Markdown, Papierkorb/Undo, Wiederholung, Archiv, Batch | KAN-041–047; atomare Recovery-/DST-Tests |
| M7 | Übersicht/Statistik, Im-/Export, Backup/Restore | KAN-048–052; Secret-Ausschluss, Epoche, Roundtrip |
| M8 | Ingress/Auth, OpenAPI, MCP/Actions, Security | KAN-053–060; 053–055 vor Netzfreigabe vorziehen |
| M9 | Manueller Updater, OS-Image, Pakete, Handbuch | KAN-061–065; Fresh-Flash/Upgrade/Rollback |
| M10 | Geräte-/Langzeit-/Security-/Lizenzabnahme, Stable | KAN-066–070; nur belegte Artefakte veröffentlichen |

## Erster überprüfbarer Schnitt

M0-Grundlage plus lokale Runtime: Migration und Seed, Boards/Spalten erstellen,
Tickets erstellen und revisionsgeprüft verschieben, WIP-Sperre, stabiler öffentlicher
Name, SQLite-Neustartpersistenz, Browserclient ohne TV-Nebenwirkung.
Keine Löschaktion, kein Offline-Versprechen und keine ungeschützte Netzwerkfreigabe.

## Danach

1. Private Gitea-Quelle und privaten GitHub-Spiegel einrichten; Commit-Identität prüfen.
2. CI ausführen und Fehler beheben. Erst dann KAN-003 als vollständig bewerten.
3. Domainkern vervollständigen: Typ-/Labelmodell, einlagige Untertickets,
   azyklische Voraussetzungen, transitive Wiederöffnung mit WIP-Backlog-Fallback.
4. Mutationshülle mit op_id, Client, Instanz/Epoche, Revision und Basissnapshot
   implementieren, ehe Wiederholungen/Offline-Queue oder weitere Adapter schreiben.
5. Debian-/Pi-Gerätezugang für systemd, Grafik, Netzwerksetup und Benchmarks nutzen,
   sobald verfügbar; fehlende Hardware wird als offener Nachweis geführt.

Remote-Issues werden mit KAN-ID, Komponente, Abhängigkeiten, Originalcheckliste,
Tests und Gate geführt. Keine automatischen Abschlussbehauptungen aufgrund
eines Dateigerüsts. Dev-/Beta-Releases dürfen Teilumfang ehrlich ausweisen.

