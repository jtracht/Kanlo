# Kanlo Plugins

Diese private Vorabversion enthält zwei ungebaute Quellpakete. Jedes ZIP
enthält eine eigene Schritt-für-Schritt-Anleitung als `README.md`.
Ein Screenshot zeigt, wo der API-Schlüssel in Kanlo angelegt wird.

- **Claude Code** lädt das entpackte Plugin lokal.
- **ChatGPT** registriert den entpackten MCP-Server über einen privaten Tunnel.

Beide Pakete benötigen Node.js ab Version 22 und eine laufende Kanlo-Instanz.
Version 0.4.0 benötigt Kanlo 0.2.0-dev.6 oder neuer. Sie kann die TV-Ansicht
wechseln, Tickets, Backlog und Untertickets einblenden, Hervorhebungen setzen
und auf der Anzeige blättern. Schreibwerkzeuge werden pro Verbindung
freigegeben.
Die Abhängigkeiten werden bei der Einrichtung mit `npm ci --omit=dev` installiert.
Die Downloads enthalten Quellcode, Konfiguration, Lizenzhinweise und Anleitungen.
`SHA256SUMS` enthält die Prüfsummen der beiden ZIP-Dateien.
