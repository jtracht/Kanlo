# Geräteabnahme für Kanlo

Die Test-SD-Karte enthält ausschließlich Testdaten. Prüfsumme und
Versionsstand des benutzten Images gehören zum Ergebnis.

## Start, Ausgänge und WLAN

1. Das ARMHF-Image auf eine freie microSD-Karte schreiben. Am Raspberry Pi
   3 B+ HDMI und USB-Tastatur anschließen. Den Zeitpunkt vom Einschalten bis
   zum sichtbaren Kanlo-Board notieren. Beim Neustart sollte dasselbe Board
   erneut erscheinen.
2. Ohne Ethernet **Strg+Alt+F2 → 5** öffnen, Smartphone oder PC mit dem
   angezeigten temporären WLAN verbinden und `http://10.42.0.1:8080` öffnen.
   Nach zehn Minuten prüfen, ob das Netz verschwunden ist. Das angezeigte
   Passwort bleibt am Gerät.
3. Über **Strg+Alt+F2 → 1** das reguläre WLAN einrichten. Die Webadresse vom
   zweiten Gerät aufrufen. Neustarten und erneut prüfen.
4. Composite mit geeignetem AV-Kabel und den Einstellungen aus der
   [Image-Anleitung](../../release/IMAGES-DE.md#hdmi-und-composite) separat
   prüfen. Falls das Board ausbleibt, **Strg+Alt+F2 → 3** öffnen und die
   erkannten Ausgänge sowie die letzten TV-Meldungen festhalten.

## Ressourcen und Dauerlast

Pro Gerät Image, Architektur, Auflösung, Anschluss und Dauer angeben.
In einer isolierten Testinstallation mindestens 1.000 Tickets, mehrere
sichtbare Spalten und zwei Tickets mit langen Markdown-Details vorbereiten.
Den gleichen Stand für Pi 3 B+ ARMHF, ein ARM64-Gerät und einen AMD64-PC
verwenden.

Das [Messskript](../../tests/images/physical_metrics.py) auf das Testgerät
kopieren und über den eingerichteten Administratorzugang starten.

```sh
sudo python3 physical_metrics.py --seconds 600 > kanlo-geraet.json
```

Während der Messung auf dem TV durch Spalten und Ticketdetails blättern,
zwei Ticketfenster öffnen, zwischen beiden Fenstern und dem Backlog wechseln
und anschließend eine Leerlaufphase stehen lassen. Bei einer Auflösung
unter 1152 × 600 Pixeln ist ein sichtbares Ticketfenster vorgesehen. Die
JSON-Datei enthält lokale HTTP-Latenzen sowie CPU- und Speicherwerte für
Server und TV-Dienst einschließlich ihrer Unterprozesse. Sichtbare Ruckler,
erste Darstellung und Anschlusswechsel separat protokollieren.
