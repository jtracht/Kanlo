# Kanlo-Server unter Debian 13

Dieses Paket enthält den Server und die Weboberfläche. Native TV-Anzeige,
Kiosk-Konfiguration und ein Flash-Image sind noch nicht enthalten.

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

Die folgenden Befehle installieren ausschließlich den Server; sie verändern
keinen Desktop und aktivieren keinen Kiosk. Aus dem entpackten Verzeichnis:

```sh
sudo install -Dm755 bin/kanlo-server /usr/local/lib/kanlo/kanlo-server
sudo install -Dm644 systemd/kanlo-server.service /etc/systemd/system/kanlo-server.service
sudo install -Dm644 sysusers.d/kanlo.conf /usr/lib/sysusers.d/kanlo.conf
sudo systemd-sysusers /usr/lib/sysusers.d/kanlo.conf
sudo systemctl daemon-reload
sudo systemctl enable --now kanlo-server
```

Der Dienst verwendet `/var/lib/kanlo`, ein nicht interaktiv nutzbares Dienstkonto
und private Verzeichnisrechte. Zustand/Fehler: `systemctl status kanlo-server`
und `journalctl -u kanlo-server`. Entfernen: Dienst stoppen/deaktivieren und die
installierten Programm-/Unit-Dateien entfernen. Daten unter `/var/lib/kanlo`
bewusst separat aufbewahren; sie werden nicht automatisch gelöscht.

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

Bei `Exec format error` die Paket-/OS-Architektur prüfen; bei Problemen mit
Display/HDMI den Server separat prüfen: Dieser Release hat keinen TV-Prozess.
