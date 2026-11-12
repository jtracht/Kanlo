# Entwicklungsstand

Der aktive Entwicklungsumfang umfasst **65 von 68 Aufgaben (95,6 %)**.
KAN-057 und KAN-060 wurden auf Wunsch aus der aktiven Planung entfernt.
Die ursprüngliche Zählung beträgt 65 von 70 Aufgaben (92,9 %).

| Zusätzlich abgenommen | Lokaler Nachweis |
| --- | --- |
| KAN-009 · Standalone-Installer | Isolierter Pakettest prüft Architektur, unveränderten Desktop im Standardmodus, gezielte Kiosk-Aktivierung, vorherige Binärdatei bei Upgrade und Datenerhalt bei Deinstallation. |
| KAN-011 · Netzwerkeinrichtung | Das AMD64-Image startet den Setup-Hotspot in QEMU. Ein separater Linux-Client verbindet sich per WPA2, erhält DHCP und lädt Kanlo. Das verkürzte Test-Zeitlimit trennt den Client und entfernt das Profil. Desktop- und Mobil-Browsertests ergänzen den Netzwerktest. |
| KAN-021 · Responsive Navigation | Board- und TV-Auswahl im Web. Desktop- und Mobiltests öffnen und bedienen die TV-Steuerung. |
| KAN-022 · Ticketmodal und TV-Aktion | Desktop und Mobil prüfen, dass das Öffnen des Webmodals die TV-Auswahl erhält und nur der ausdrückliche TV-Befehl eine Einblendung setzt. |
| KAN-027 · Grafik auf Pi 3 B+ | Der Anwender bestätigte das lokal sichtbare Board auf dem ARM32-Gerät. Docker prüfte die sichtbare Weston-Ausgabe mit denselben ARM-Paketen. |
| KAN-029 · Native Ticketfenster | Markdown, Tabellen, Code, Kommentare und optionale begrenzte HTTPS-Bilder. Tests prüfen unabhängige Dauer, Wechsel auf zwei bzw. ein Fenster, beide Scrollpositionen nach erzwungenem Absturz und 600 Sekunden Wayland-Dauerlauf. |
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
| KAN-062 · Raspberry-Pi-Flash-Image | Der Anwender bestätigte SD-Erststart, WLAN-Liste, Board und Neustart mit dem [Weston-Image](PI3-ABNAHME.md). |
| KAN-070 · Offene Folgearbeiten | [Offene Originalabnahmen](../planning/OFFENE-ABNAHME.md) und getrennte lokale Post-1.0-Tickets. |

Die [offenen Originalabnahmen](../planning/OFFENE-ABNAHME.md) umfassen
weiterhin die Dauerlast auf den Zielgeräten und die Stable-Freigabe.
