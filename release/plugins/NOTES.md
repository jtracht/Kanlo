# Kanlo MCP 0.6.0

Der Adapter liest beim Start den OpenAPI-Vertrag von Kanlo.
Die Einstellungswerkzeuge lesen Kategorien, Namen in Fünferseiten und
ausdrücklich angeforderte Werte. Für Board-Regeln mit Board-Auswahl ist
Kanlo 0.2.0-dev.10 erforderlich. Mit Kanlo 0.2.0-dev.8 kann jedes Ticketfenster ein eigenes TV-Zeitlimit erhalten. Neu sind die
exakte Titelsuche mit Anzeige mehrerer Treffer und der Rückgängig-Verlauf mit
Vorschau und separater Bestätigung für freigegebene Schreibzugriffe.

## Quellpakete

Diese private Vorabversion enthält zwei ungebaute Quellpakete. Jedes ZIP
enthält eine eigene Schritt-für-Schritt-Anleitung als `README.md`.
Ein Screenshot zeigt, wo der API-Schlüssel in Kanlo angelegt wird.

- **Claude Code** lädt das entpackte Plugin lokal.
- **ChatGPT** registriert den entpackten MCP-Server über einen privaten Tunnel.

Beide Pakete benötigen Node.js ab Version 22 und eine laufende Kanlo-Instanz.
Version 0.6.0 benötigt Kanlo 0.2.0-dev.6 oder neuer. Sie kann die TV-Ansicht
wechseln, Tickets, Backlog und Untertickets einblenden, Hervorhebungen setzen
und auf der Anzeige blättern. Schreibwerkzeuge werden pro Verbindung
freigegeben.
Die Abhängigkeiten werden bei der Einrichtung mit `npm ci --omit=dev` installiert.
Die Downloads enthalten Quellcode, Konfiguration, Lizenzhinweise und Anleitungen.
`SHA256SUMS` enthält die Prüfsummen der beiden ZIP-Dateien.
