# Kanlo-Server unter Debian 13

Dieses Paket enthält den Server und die Weboberfläche für vorhandene
Desktop- oder Headless-Systeme. Das separate [Geräte-Image](IMAGES-DE.md)
enthält die native Kiosk-Anzeige und die lokale WLAN-Einrichtung.

1. Passendes Paket wählen: `amd64` für x86-64-PCs, `arm64` für ein 64-Bit-ARM-OS,
   `armhf` für ein ARMv7-Hard-Float-OS. Die OS-Architektur zählt, nicht nur die CPU.
   Physische Raspberry-Pi-Abnahmen stehen noch aus.
2. `sha256sum -c SHA256SUMS` im Downloadverzeichnis ausführen und das Archiv entpacken.
3. Im entpackten Verzeichnis als normaler Benutzer starten:

   ```sh
   mkdir -p "$HOME/.local/share/kanlo"
   KANLO_DATA_DIR="$HOME/.local/share/kanlo" ./bin/kanlo-server
   ```

4. `http://127.0.0.1:8080` öffnen. Kein Bildschirm und kein Internetzugang sind
   für den laufenden Server nötig. Mit Strg+C beenden. `--check` prüft die Datenbank.

## Optionaler Systemdienst

Das Skript prüft Betriebssystem, Architektur und die Prüfsumme der
Serverdatei. Es installiert den Server als Dienst im vorhandenen System.
Aus dem entpackten Verzeichnis:

```sh
./install.sh --check
sudo ./install.sh
```

Der Dienst verwendet `/var/lib/kanlo`, ein nicht interaktiv nutzbares Dienstkonto
und private Verzeichnisrechte. Zustand und Fehler stehen in
`systemctl status kanlo-server` und `journalctl -u kanlo-server`. Ein Upgrade
bewahrt die bisherige Binärdatei als `kanlo-server.previous` auf. Zum
Entfernen `sudo ./install.sh --uninstall` aus dem Paket ausführen. Die Daten
und Einstellungen bleiben auf dem System.

### Optionale native Anzeige

Auf einem dedizierten Linux-System mit freier lokaler Konsole kann der
Serverinstaller auch die Anzeige installieren.

```sh
sudo apt install cage seatd python3-pygame python3-markdown-it python3-pil fonts-dejavu-core
sudo ./install.sh --kiosk
```

Die Anzeige nutzt tty1. Ein aktiver Desktop-Anmeldedienst muss für diesen
Modus beendet sein. Für die WLAN-Einrichtung am Bildschirm das Geräte-Image
verwenden.

## Netzwerk, Sicherung und Wartung

Standard ist nur Loopback. LAN benötigt `KANLO_BIND` und genaue erlaubte Hosts;
externes HTTPS einen eigenen geschützten Loopback-Listener hinter einem Proxy.
Details stehen in `docs/de/BETRIEB.md` im Paket. Für Offline-PWA auf anderen
Geräten ist vertrauenswürdiges HTTPS nötig; LAN-HTTP bleibt online nutzbar.

Vor einem manuellen Update ein Datenbackup über die Oberfläche herunterladen,
Dienst stoppen, bisheriges Programmpaket aufbewahren, neue Datei installieren
und neu starten. Eine ältere Binärdatei darf eine neuere Datenbankschemaversion
nicht öffnen. Für einen Daten-Rollback sind das passende alte Programm und ein
vorheriges Backup erforderlich; es gibt noch keinen automatischen Updater.

Bei `Exec format error` die Paket-/OS-Architektur mit `dpkg --print-architecture`
prüfen. Für HDMI und Composite auf dem Pi 3 B+ siehe [Geräte-Images](IMAGES-DE.md).

## Kanlo-Server manuell aktualisieren

Das neue Serverpaket und seine SHA-256-Prüfsumme aus derselben vertrauenswürdigen
Releasequelle beziehen. Für das installierte Kanlo mit Standard-Datenverzeichnis
`/var/lib/kanlo` und Python 3:

```sh
sudo python3 /usr/lib/kanlo/update.py apply /pfad/kanlo-VERSION-linux-ARCH.tar.gz --sha256 SHA256
sudo python3 /usr/lib/kanlo/update.py rollback
```

`apply` stoppt den Dienst kurz, sichert Programm und SQLite-Daten, probiert
die Migration auf einer Kopie und prüft nach dem Start die Paketversion.
Bei einem Fehler stellt es den vorherigen Zustand wieder her. Der separate
Befehl `rollback` stellt die Daten vom Zeitpunkt vor dem Update wieder her.
Später gespeicherte Änderungen gehen dabei verloren. Clients mit ausstehenden
Offline-Änderungen werden durch die neue Datenbank-Epoche zur Prüfung aufgefordert.
Die private Rollback-Kopie unter `/var/lib/kanlo-updates` enthält auch
Authentifizierungsdaten und muss unter Root-Zugriff bleiben.
