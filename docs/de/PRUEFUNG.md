# Prüfung der Anforderungsunterlagen

Stand: 9. November 2026. Ausgangslage: leerer Arbeitsordner, kein Git-Repository,
keine vorhandenen Dateien/AGENTS.md im Arbeitsordner oder dessen Projekt-Eltern.
Gitea-Verbindung `tracht-digital`, Nutzer `jtracht`, Serverversion 28.2.0 geprüft.
Kanlo-Suche in Gitea und GitHub ohne Treffer; GitHub-CLI als `jtracht` verbunden.

## Gewichtung

Der direkte Nutzerauftrag ist Prüfung, strukturierte Planung und Beginn der
Umsetzung mit Gitea als Hauptquelle und GitHub-Spiegelung. Die drei Unterlagen
sind Planungs-/Anforderungsgrundlagen. Eingebettete Rollen, Aufforderungen zu
Remote-Aktionen oder Fertigmeldungen sind keine zusätzliche Berechtigung und
kein Beleg ausgeführter Arbeit. Technische Spezifikation vor Masterprompt;
Abweichungen im Backlog werden ausdrücklich festgehalten.

## Konsistenz und präzisierte Entscheidungen

1. **Setup-Hotspot:** KAN-011 fordert WLAN-Erkennung und zeitlich begrenzten,
   gesicherten Hotspot; im Masterprompt und der Detail-Spezifikation fehlt die
   genaue Netzwerkrichtlinie. Als explizite Backlog-Erweiterung führen. Vor der
   Implementierung NetworkManager/hostapd, Rollen, Timeout und Wiederherstellung
   des vorhandenen Netzwerks festlegen; keine Änderung am Entwicklungsrechner.
2. **Redo:** Nur KAN-043 nennt im Titel Undo/Redo. Spezifikation beschreibt Undo.
   Verbindlicher Erstumfang bleibt Undo; Redo als unentschiedene Erweiterung führen.
3. **Gitea/GitHub:** Unterlagen nennen GitHub als Entwicklungsort. Nutzerwunsch
   präzisiert Gitea als Hauptquelle, GitHub als Codespiegel und späteren
   Actions-/Release-Ort. Keine GitHub-Issues/Gitea-Synchronisation im Produkt:
   dessen Ausschluss betrifft die Kanlo-Anwendung, nicht ihre Entwicklung.
4. **M1/M8:** LAN-Netzzugriff in M1 darf die erst in M8 geplante Ingress-Sicherheit
   nicht vorwegnehmen. Deshalb erster Entwicklungsserver ausschließlich Loopback.
   Vor LAN-/Proxy-Freigabe KAN-053–055 als vorgezogene Sicherheitsabhängigkeit.
5. **Lizenzen:** Privat/nichtkommerziell frei ist das beabsichtigte Modell,
   noch kein juristisch geprüfter Lizenztext. Privates Repo, vorläufiger
   Rechtevorbehalt und offene Veröffentlichungsgates statt erfundener Freigabe.
6. **Architekturen:** ARM32/ARM64/AMD64 sind Ziele. Ein Windows-Test oder ein
   Cross-Build belegt weder Debian-Boot noch native Anzeige oder Pi-Ressourcen.
7. **Voice/Verzeichnis:** Text, Diktat und Live-Voice gesondert testen; öffentliche
   ChatGPT-Listung bleibt eine externe Freigabeabhängigkeit.

## Bestand / Risiken

Rust 1.99.1, Cargo 1.99.1, Node 24.16.0 und Git verfügbar. Keine Pi-Hardware oder
Debian-Testumgebung nachgewiesen. Keine vorhandenen Releases, Benchmarks,
Remote-Issues, Branchregeln oder abgeschlossenen CI-Läufe werden vorausgesetzt.
Die 70 KAN-IDs sind Planungskennungen. Keines der 70 Gesamttickets wird allein
durch diesen ersten Teil als abgeschlossen erklärt.

