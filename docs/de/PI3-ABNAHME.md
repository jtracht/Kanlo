# Raspberry Pi 3 B+ · Start und lokale Anzeige

Am 12. November 2026 bestätigte der Anwender den Test mit dem
Weston-Image auf dem Pi 3 B+. Die Rückfrage umfasste den Erststart von SD,
die WLAN-Liste, das sichtbare Kanlo-Board und den Neustart. Rückmeldung:
„alles klappt“. Der verwendete Bildschirmanschluss und Zeiten wurden
dabei nicht einzeln protokolliert.

| Prüfstand | Ergebnis |
| --- | --- |
| Physischer Pi 3 B+ | SD-Erststart, WLAN-Liste, lokales Board und Neustart vom Anwender bestätigt. |
| ARMHF-Image in QEMU | Erststart und Neustart bestanden. Geprüft wurden auch HTTP auf Port 80, SQLite und unabhängige Dienstneustarts. |
| ARMHF-Pakete aus demselben Image in Docker | Weston und der GTK/Wayland-Renderer lieferten ein sichtbares Board im virtuellen Display. |

Geprüft wurde `kanlo-0.2.0-dev.10-rpi-armhf.img.xz` aus Commit
`6b47c7f`. Die SHA-256-Prüfsumme des komprimierten Images lautet
`56cde72112fe271c1cab157606b4b833570908e22db3c29999835abb16b2cc0d`.

Die Rückmeldung deckt die nutzbare lokale Anzeige (KAN-027) und die
Erstinstallation auf SD (KAN-062) ab. Einzelne Anschlussprotokolle und
Messwerte zur [Dauerlast auf den Zielgeräten](GERAETEABNAHME.md) stehen aus.
