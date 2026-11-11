# Kanlo Plugins

Diese private Vorabversion enthält zwei ungebaute Quellpakete. Jedes ZIP
enthält eine eigene Schritt-für-Schritt-Anleitung als `README.md`.
Ein Screenshot zeigt, wo der API-Schlüssel in Kanlo angelegt wird.

- **Claude Code** lädt das entpackte Plugin lokal.
- **ChatGPT** registriert den entpackten MCP-Server über einen privaten Tunnel.

Beide Pakete benötigen Node.js ab Version 22 und eine laufende Kanlo-Instanz.
Version 0.3.0 benötigt Kanlo 0.2.0-dev.3 oder neuer. Sie kann Kommentare
seitenweise abrufen und bei freigegebenem Schreibzugriff Tickets in Zielspalten
oder ins Backlog verschieben. Die boardübergreifende Suche, Bearbeitung und
Kommentarfunktion sind weiterhin verfügbar.
Die Abhängigkeiten werden bei der Einrichtung mit `npm ci --omit=dev` installiert.
Die Downloads enthalten Quellcode, Konfiguration, Lizenzhinweise und Anleitungen.
`SHA256SUMS` enthält die Prüfsummen der beiden ZIP-Dateien.
