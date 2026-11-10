# Kanlo-Geräte-Images

Die Images richten ein **dediziertes Kanlo-Gerät** ein. Bestehende Installationen
werden damit vollständig ersetzt. Für einen vorhandenen Desktop stattdessen das
Serverpaket verwenden. Images bleiben private Entwicklungsversionen.

| Image | Ziel |
|---|---|
| rpi-armhf | Pi 3 B/B+, Pi 4/400, ARMv7/32 Bit; bevorzugt für 1 GiB RAM |
| rpi-arm64 | Pi 3 und neuer, Pi 4/5/400/500, Zero 2 W; 64 Bit |
| generic-amd64 | x86-64-PC/Mini-PC oder VM mit UEFI und Debian-kompatibler Hardware |
| generic-arm64 | ARM64-UEFI-/SBSA-Gerät oder VM; kein universelles Image für beliebige ARM-Boards |

Pi 1 und ursprünglicher Pi Zero sind wegen des ARMv7-Servers ausgeschlossen.
Generic ARM64 ist kein Apple-Silicon- oder herstellerspezifisches SBC-Image.
UEFI Secure Boot ist nicht als unterstützt abgenommen. Physische HDMI-/WLAN-/
Boot-Abnahmen auf den genannten Geräten stehen noch aus; die Pipeline testet
echte Kernel-/systemd-Boots unter QEMU, keine simulierten Erfolgsmeldungen.

## Flashen und erster Start

1. Image, zugehörige `.image.json` und `SHA256SUMS` herunterladen. Den Hash des
   gewählten Images gegen `SHA256SUMS` prüfen. `sha256sum -c SHA256SUMS` prüft
   alle Dateien, wenn der gesamte Release heruntergeladen wurde.
2. Mindestens **16 GB** SD-Karte/USB/SSD bereitstellen. Mit Raspberry Pi Imager
   „Eigenes Image“ oder einem anderen Raw-Image-Flasher `.img.xz` auf den
   ausdrücklich gewählten Zieldatenträger schreiben. **Alle dortigen Daten
   gehen verloren.** Die Buildskripte selbst flashen keine physischen Geräte.
3. Optional die FAT-Bootpartition öffnen. `KANLO_CONFIG.example.json` nach
   `KANLO_CONFIG.json` kopieren und anpassen. Beispiel:

   ```json
   {
     "hostname": "kanlo-buero",
     "timezone": "Europe/Berlin",
     "display": true,
     "wifi": {"ssid": "Mein WLAN", "password": "mein-eigenes-langes-kennwort", "country": "DE"},
     "ssh_authorized_key": null
   }
   ```

   Das ist das eigene WLAN-Kennwort, kein mitgeliefertes Kanlo-Kennwort.
   WLAN/SSH-Einträge bei Nichtbedarf auf `null` lassen. SSH akzeptiert genau
   einen eigenen Public Key, keine privaten Schlüssel. Die echte Konfigurations-
   datei wird nach erfolgreicher Übernahme von der Bootpartition gelöscht.
   Bei UEFI-Images ist dies die EFI-Systempartition; Windows zeigt sie häufig
   nicht automatisch als Laufwerk an. In diesem Fall die lokale Einrichtung
   unter Ctrl+Alt+F2 verwenden oder die gewählte EFI-Partition unter Linux öffnen.
4. Gerät mit Ethernet oder konfiguriertem WLAN starten. Die Datenpartition
   erweitert sich automatisch auf den Datenträger. Kanlo startet ohne Anmeldung
   und ohne Internet. Ohne Konfiguration heißt das Gerät `kanlo-<Gerätekennung>`.
5. Die Webadresse erscheint auf dem angeschlossenen Display bzw. unter
   Ctrl+Alt+F2 auf der lokalen Konsole. Im selben LAN
   `http://<hostname>.local:8080` oder `http://<IPv4-Adresse>:8080` öffnen.
   mDNS muss im Clientnetz erlaubt sein; sonst die DHCP-Liste des Routers nutzen.

Ctrl+Alt+F2 öffnet den lokalen Einrichtungsdialog; Ctrl+Alt+F1 kehrt zur Anzeige
zurück. Der Dialog erlaubt dem physisch anwesenden Gerätebesitzer Netz-/Host-/
Anzeigeeinstellungen. Es gibt keine offene Root-Shell und kein gemeinsames
Administratorpasswort. Ein ausdrücklich eingerichteter SSH-Key erhält volle
Geräteverwaltung als `kanlo-admin` mit sudo; ohne Schlüssel bleibt SSH aus.

## Verhalten und Optimierungen

- Server und native Anzeige sind getrennte, automatisch startende Dienste.
  Ohne angeschlossenen Bildschirm bleibt der Server verfügbar. DRM-Hotplug
  startet die Anzeige erneut. Kein vollständiger Desktop und kein Browsermotor.
- Native erste Boardansicht mit Ticketkarten und wechselnden Seiten/Boards.
  Bearbeitung erfolgt im Web. Erweiterte TV-Funktionen sind weiterhin Entwicklung.
- SQLite unter `/var/lib/kanlo`, Dienstnutzer `kanlo`, Rechte 0700. Daten und
  Einstellungen bleiben bei Neustarts erhalten. Kein flüchtiges Root-Overlay,
  das Daten oder Updates beim Ausschalten verwirft.
- Logs flüchtig, maximal 32 MiB. ZRAM maximal 512 MiB bzw. halber RAM; kein
  persistenter SD-Swap. Kein Overclocking, keine abgeschalteten SQLite-Fsyncs.
- LAN-Zugang bleibt an exakte lokale Hosts/IPv4-Adressen und private Gegenstellen
  gebunden. Kein Internet-Portforwarding auf diesen LAN-Port einrichten.
  Externen Zugang und Offline-PWA auf anderen Geräten ausschließlich über den
  getrennten geschützten HTTPS-Zugang aus `docs/de/BETRIEB.md` einrichten.

## Wartung und Fehlerbehebung

Vor Update/Reflash ein Kanlo-Datenbackup herunterladen und sicher aufbewahren.
Reflash löscht Daten; es gibt noch keinen automatischen A/B-Image-Updater oder
automatischen Daten-Rollback. OS-Sicherheitsupdates können nach optionaler
SSH-Einrichtung mit Debian-Werkzeugen vorgenommen werden. Nicht ungesichert
das laufende Datenbankdateisystem kopieren oder im Betrieb abschalten.

Nach SSH-Einrichtung: `systemctl status kanlo-server kanlo-firstboot kanlo-tv`,
`journalctl -b -u kanlo-server -u kanlo-firstboot -u kanlo-tv`. Logs sind nach
einem Neustart absichtlich leer. Bei falscher Konfiguration lokal Ctrl+Alt+F2
verwenden. Bei fehlender Anzeige zunächst Webzugriff testen; Hardwaretreiber,
Kabel und unterstützte Grafik prüfen. Bei fehlendem WLAN Land/SSID/Kennwort
prüfen oder für die Einrichtung Ethernet anschließen.

## Bau und Nachweise

`packaging/images/bases.json` pinnt offizielle Basisimages inklusive Hash.
Der Build installiert alle Laufzeitpakete, dokumentiert sie und legt Kanlo mit
passender Architektur und Quellstand ein. Der erste Start benötigt keine
Downloads; normale OS-Updatefunktionen bleiben davon getrennt. Die native Anzeige wird zusätzlich auf einem echten
headless Wayland-Compositor getestet. Das zu veröffentlichende Image bleibt
unberührt: Zwei Bootprüfungen laufen auf einer separaten Kopie, prüfen ein
geschriebenes Ticket nach Reboot und kontrollieren SQLite abschließend.
Für ARM-Boots wird ein gepinnter QEMU-Upstream-Stand mit vollständigem BCM2835-
Watchdog-Modell gebaut und zwischengespeichert. Die unveränderten Pi-Watchdog-
Einstellungen bleiben auch im Boot-Test aktiv. Emulatorversion, Quellstand und
Binärhash stehen im Nachweis; die Korrektur ist kein Bestandteil des Geräte-OS.
Der ausschließlich für den Pi-Test kopierte Gerätebaum verwendet SD-PIO; die
Test-Kommandozeile nutzt reguläre USB-Interrupts. Die entsprechenden DMA-/FIQ-
Pfade sind im Emulator nicht originalgetreu abgebildet. Ausgelieferte Firmware
und Konfiguration behalten die Herstellervorgaben. Damit werden keine reale
Speicher-/Netzwerkleistung und keine HDMI-/WLAN-Hardware zertifiziert.
Beide Bootdurchläufe prüfen außerdem DHCP/LAN-HTTP und begrenzten RAM-Swap.

Die Pipeline baut alle vier Varianten automatisch. Jeder Versions-Tag verlangt
alle erfolgreichen Qualitäts-, Plattform-, Image- und Bootprüfungen; erst dann
werden Images, Nachweise und Prüfsummen veröffentlicht.

Quellen: [Raspberry Pi OS](https://www.raspberrypi.com/software/operating-systems/),
[Debian-Images](https://cloud.debian.org/images/cloud/trixie/),
[Cage](https://github.com/cage-kiosk/cage),
[NetworkManager-Keyfiles](https://networkmanager.dev/docs/api/latest/nm-settings-keyfile.html).
Emulator-Korrektur: [QEMU-Quellstand](https://github.com/qemu/qemu/commit/7e0b2296c5d8e6ae0325f32c2c879ca2aba83ed5).
