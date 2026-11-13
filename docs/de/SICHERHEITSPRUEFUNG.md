# Lokale Sicherheits- und Datenschutzprüfung

Geprüfter Entwicklungsstand 0.2.0-dev.6. Diese Prüfung betrifft den
Anwendungsquellcode, die verwalteten Abhängigkeiten und lokale Testsysteme.

| Bereich | Prüfung |
| --- | --- |
| HTML und Skripte | Der Browser rendert Markdown über den bestehenden Sanitizer. Browsertests prüfen unsichere Links, eingebetteten Code und das Verhalten der Ticketansicht. HTTP-Tests prüfen lokale Assets und CSP. |
| Anfragen an externe URLs | Die Rust-API lädt keine URLs aus Tickets oder Kommentaren. Die TV-Anzeige fragt ausschließlich die lokale Kanlo-API ab und zeigt verlinkte Ticketkennungen als Text. Der MCP-Adapter nutzt die vom Betreiber konfigurierte Origin und lehnt Weiterleitungen ab. |
| Authentifizierung und CSRF | HTTP-Tests prüfen LAN- und Proxy-Trennung, Host- und Origin-Prüfung, Sitzungsablauf, Schutz für TV-Aktionen und widerrufbare API-Schlüssel. Der Schlüsseltest erfordert auch im LAN einen gültigen Bearer-Schlüssel. |
| Eingabe und Speicher | Parametrisierte SQLite-Abfragen, validierte IDs, Bodygrenzen und Domainprüfungen. Tests prüfen unter anderem ungültige Eingaben, Formelzellen in CSV und Pfade in Release-Archiven. |
| Offline und Telemetrie | Der Server funktioniert im Container ohne Netzwerk außer Loopback. Die App lädt ihre Laufzeitassets lokal. Nur besuchte Boards gelangen in den Browsercache. Absichtlich angezeigte externe Markdown-Bilder können Anfragen des Browsers an den Bildhost auslösen. |
| Zugangsdaten | Tests prüfen, dass Sicherungen weder Sitzungstokens noch API-Schlüssel oder Passcodehashes enthalten. Der Browser leert den Schlüsseltest nach dem Absenden und exportiert das GPT-Schema ohne Schlüssel. |
| Abhängigkeiten | npm-Audits der Web-, Plugin- und Browserpakete lieferten keine Befunde. Der lokale Trivy-Scan meldete keine High- oder Critical-Befunde in Cargo.lock und den Produktions-npm-Lockfiles. |

Die Prüfung deckt keine externen Reverseproxies, ChatGPT-Workspaces oder
Produktionsnetze ab. Diese müssen jeweils mit ihrer tatsächlichen
Konfiguration geprüft werden. Das vollständige Stable-Freigabegate bleibt
separat.
