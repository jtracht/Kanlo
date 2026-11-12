# Entwicklungsstand

Der lokale Stand 0.2.0-dev.6 umfasst **60 von 70 Originalaufgaben
(85,7 %)**. Er baut auf den 42 Aufgaben der [60-Prozent-Abnahme](60-PROZENT.md)
auf. Das Ziel von mindestens 85 % liegt bei 60 Aufgaben.

| Zusätzlich abgenommen | Lokaler Nachweis |
| --- | --- |
| KAN-009 · Standalone-Installer | Isolierter Pakettest prüft Architektur, unveränderten Desktop im Standardmodus, gezielte Kiosk-Aktivierung, vorherige Binärdatei bei Upgrade und Datenerhalt bei Deinstallation. |
| KAN-021 · Responsive Navigation | Board- und TV-Auswahl im Web. Desktop- und Mobiltests öffnen und bedienen die TV-Steuerung. |
| KAN-022 · Ticketmodal und TV-Aktion | Desktop und Mobil prüfen, dass das Öffnen des Webmodals die TV-Auswahl erhält und nur der ausdrückliche TV-Befehl eine Einblendung setzt. |
| KAN-030 · Backlog-Einblendung | API-Test für Backlog → Ticket → Backlog, Ablaufprüfung und nativer Rendertest. Die Anzeige merkt sich die Seitenposition des Backlogs beim Öffnen eines Tickets. |
| KAN-031 · Untertickets und Elternkontext | Domänentest für Elternansicht, Unterticket, Ablauf und Ersetzen des Kontexts durch ein fremdes Ticket. Native Unterticketansicht im SDL-Test. |
| KAN-032 · Highlights und TV-Scrollen | API-Test für Hervorhebung und Seitenwechsel. Layouttest für vertikalen Überlauf pro Spalte vor dem horizontalen Wechsel. Auto-Scroll bleibt bei Einblendungen aus. |
| KAN-033 · Ansichtswechsel und Rotation | Auswahl von Boards, Gesamtübersicht und Statistik mit gespeicherter Reihenfolge und Intervall. SQLite-Neustarttest für die gespeicherte Konfiguration. |
| KAN-041 · Kommentare und Ticketlinks | Kommentare und verknüpfte Ticketkennungen im nativen Ticketfenster zusätzlich zu den bestehenden API- und Webfunktionen. |
| KAN-047 · Deadline- und WIP-Hinweise | Nativer Hinweis für das Erreichen eines WIP-Limits und Test für den Unterschied zwischen Datum und Zeitpunkt bei Fälligkeit. |
| KAN-048 · Globale Übersicht | Web und TV nutzen dieselbe boardübergreifende Kennzahlen-API. Der HTTP-Test prüft offene Tickets und Backlog, das Web verlinkt das zugehörige Board. |
| KAN-049 · Historische Statistik | Bestehender Test für Statistik bei Undo-Tiefe 0, hinzugekommen sind Tagesaggregation und nativer Diagrammtest. |
| KAN-058 · Integrationsassistent | Getrennte Anleitungen für MCP und GPT Actions auf Deutsch und Englisch. Desktop und Mobil prüfen den Schlüsseltest und den Export ohne Zugangsdaten. |
| KAN-059 · Sicherheits- und Datenschutzprüfung | [Lokale Prüfmatrix](SICHERHEITSPRUEFUNG.md) mit HTTP-, Browser- und Abhängigkeitstests. |
| KAN-070 · Offene Folgearbeiten | [Offene Originalabnahmen](../planning/OFFENE-ABNAHME.md) und getrennte lokale Post-1.0-Tickets. |

Die [offenen Originalabnahmen](../planning/OFFENE-ABNAHME.md) umfassen
weiterhin den physischen Raspberry-Pi-Start samt HDMI-, Composite- und WLAN-Prüfung,
vollständige native Markdown- und Bilddarstellung, den Update-Rollback und die
ChatGPT-Abnahme im tatsächlichen Workspace.
