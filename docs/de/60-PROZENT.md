# Entwicklungsstand 60 %

Der Release 0.2.0-dev.5 umfasst **42 von 70 vollständig abgenommenen
Originalaufgaben**. Gezählt werden Aufgaben, keine Aufwandsanteile.
Die [erste Abnahme mit 35 Aufgaben](50-PROZENT.md) bleibt als historischer
Nachweis erhalten. Diese sieben Aufgaben kommen mit ihren jeweiligen
Prüfnachweisen hinzu.

| Aufgabe | Nachweis |
| --- | --- |
| KAN-008 · systemd und Neustart | Der Gasttest beendet Server und Anzeige nacheinander und prüft, dass nur der beendete Dienst eine neue PID erhält. |
| KAN-010 · Display-Erkennung und Headless-Start | Anschlussprüfungen decken HDMI, Composite und fehlende Modi ab. Der Gasttest bootet ohne aktivierte Anzeige und prüft LAN-Zugriff, Reboot und das spätere Starten der separaten Anzeige. |
| KAN-028 · Native Boards und Displayformate | Der SDL-Test zeichnet echte Ticketkarten im kleinen PAL-, Hochkant-, Quer- und Ultrawide-Format. Der Wayland-Test prüft dieselbe Ansicht, die Tests erfassen den Spitzenverbrauch und die Anzeige zeichnet bei Netzstatusänderungen nur den Fußbereich neu. |
| KAN-040 · Echtzeit und Reconnect | Web- und TV-Clients verwenden denselben Ereignisstrom. Der Gasttest erstellt ein Ticket, beobachtet es im TV-Client und prüft die erneute Zustellung nach dem Serverneustart. |
| KAN-063 · Plattformpakete und Prüfsummen | Debian-13-Serverpakete für AMD64, ARM64 und armhf mit Installationsskript, Git-Commit und Binärprüfsumme. Die Veröffentlichung prüft zusätzlich die Geräte-Images und verifiziert jeden Upload. |
| KAN-064 · Tag-basierte Release-Gates | Die Release-Pipeline läuft gemäß Betriebsentscheidung auf Gitea. Dev-, Beta- und RC-Tags bleiben Vorabversionen. Der Stable-Gate-Test weist einen Stable-Tag ohne Freigaben ab. |
| KAN-065 · Installations- und Betriebshandbuch | Deutsche und englische Anleitungen für Serverinstallation, Geräte-Image, LAN/HTTPS/PWA, Sicherung und Display-Fehlersuche. Die Paketprüfung führt `install.sh --check` aus. |

Maßgebliche Prüfschritte stehen in `tests/images/`, `tests/release/`,
`packaging/images/boot_guest.py` und `.github/workflows/ci.yml`.
Die Image-Berichte des Releases enthalten je Profil die beiden Bootnachweise,
den Neustarttest und den Live-Display-Test.

Die physische Pi-Abnahme für HDMI, Composite, WLAN und SD-Karten bleibt offen.
Die 60 % setzen auch keine vollständige TV-Overlay-Steuerung, öffentliche
Plugin-Freigabe oder Stable-Freigabe voraus.
