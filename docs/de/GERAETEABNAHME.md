# Geräteabnahme für Kanlo

Die Test-SD-Karte enthält ausschließlich Testdaten. Prüfsumme und
Versionsstand des benutzten Images gehören zum Ergebnis.

## Start, Ausgänge und WLAN

1. Das ARMHF-Image auf eine freie microSD-Karte schreiben. Am Raspberry Pi
   3 B+ HDMI und USB-Tastatur anschließen. Den Zeitpunkt vom Einschalten bis
   zur WLAN-Ersteinrichtung und anschließend zum sichtbaren Kanlo-Board
   notieren. Beim Neustart sollte das Board direkt erscheinen.
2. Ohne Ethernet **Strg+Alt+F2 → WLAN → 3** öffnen, Smartphone oder PC mit dem
   angezeigten temporären WLAN verbinden und `http://10.42.0.1` öffnen.
   Nach zehn Minuten prüfen, ob das Netz verschwunden ist. Das angezeigte
   Passwort bleibt am Gerät.
3. Über **Strg+Alt+F2 → WLAN → 1** das reguläre WLAN einrichten. Die Webadresse vom
   zweiten Gerät aufrufen. Neustarten und erneut prüfen.
4. Composite mit geeignetem AV-Kabel und den Einstellungen aus der
   [Image-Anleitung](../../release/IMAGES-DE.md#hdmi-und-composite) separat
   prüfen. Falls das Board ausbleibt, **Strg+Alt+F2 → 3** öffnen und die
   erkannten Ausgänge sowie die letzten TV-Meldungen festhalten.

5. Über **Strg+Alt+F2 → Bluetooth** den Status prüfen und ein verfügbares
   Zubehör koppeln, verbinden und wieder entfernen.

## Automatisierter WLAN-Test

Das AMD64-Image verwendet im QEMU-Test zwei virtuelle Linux-WLAN-Adapter.
Ein separater Client verbindet sich per WPA2, erhält DHCP und liest
die Kanlo-Startseite und den initialen Board-Stand. Der Test setzt das
Zeitlimit im isolierten Gast von zehn Minuten auf 30 Sekunden und prüft,
dass die Verbindung getrennt und das temporäre Profil entfernt wird.
Die Prüfung ist Bestandteil der Image-Pipeline.
[Linux-WLAN-Simulation](https://wireless.docs.kernel.org/en/latest/en/users/drivers/mac80211_hwsim.html)

## Ressourcen und Dauerlast

Pro Gerät Image, Architektur, Auflösung, Anschluss und Dauer angeben.
In einer isolierten Testinstallation das leere Board `Geräteabnahme` mit den drei
Spalten `Offen`, `In Arbeit` und `Erledigt` anlegen. Die [Testdaten](../../tests/images/physical_workload.py)
erzeugen 1.200 Tickets, davon je 300 in jeder Spalte und 300 im Backlog.
Die ersten beiden Tickets enthalten lange Markdown-Details.

```sh
python3 tests/images/physical_workload.py kanlo-dauerlast.csv --columns Offen "In Arbeit" Erledigt
```

Die CSV im Web über den Importdialog für das Testboard öffnen, die
Vorschau mit 1.200 neuen Tickets prüfen und bestätigen. Dieselbe Datei für
Pi 3 B+ ARMHF, ein ARM64-Gerät und einen AMD64-PC verwenden.

Auf einem aktuellen Geräte-Image das mitgelieferte
[Messskript](../../tests/images/physical_metrics.py) über den eingerichteten
Administratorzugang starten. Bei älteren Images kann die Datei aus dem
Repository kopiert werden.

```sh
sudo python3 /usr/share/kanlo/diagnostics/physical_metrics.py --seconds 600 --board Geräteabnahme > kanlo-geraet.json
```

Während der Messung auf dem TV durch Spalten und Ticketdetails blättern,
zwei Ticketfenster öffnen, zwischen beiden Fenstern und dem Backlog wechseln
und anschließend eine Leerlaufphase stehen lassen. Bei einer Auflösung
unter 1152 × 600 Pixeln ist ein sichtbares Ticketfenster vorgesehen. Die
JSON-Datei enthält lokale HTTP-Latenzen sowie CPU- und Speicherwerte für
Server und TV-Dienst einschließlich ihrer Unterprozesse. Sie enthält auch
das vom System gemeldete Gerätemodell, die Build-ID des installierten Images,
die Ticketanzahl des Testboards und die während der Messung abgerufenen
Einblendungsarten.
Zu Beginn und am Ende erfasst sie den Renderer, der das erste Bild gemeldet
hat, die aktive Konsole sowie Anschlussstatus und angebotene Auflösungen. Die
tatsächlich sichtbare Anzeige, Ruckler, erste Darstellung und Anschlusswechsel
separat protokollieren.

Im lokalen Docker-Test vom 12. November 2026 lief die Weston-Anzeige bei
1920 × 1080 mit 1.200 Tickets für 600 Sekunden. Nach dem erzwungenen
Prozessende stellte sie die Position beider Ticketfenster wieder her. Der
Renderer erreichte im Test maximal 161,2 MiB RSS. Das Ergebnis gehört zur
virtuellen Testumgebung und ergänzt die Messungen auf den Zielgeräten.
