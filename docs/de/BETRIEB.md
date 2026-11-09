# Entwicklungsbetrieb

Das Programm bettet sämtliche Webassets ein. Es benötigt zur Laufzeit weder
Node noch Docker, CDN, Internet oder Bildschirm. Eine native TV-Laufzeit ist
noch nicht vorhanden; keine TV-Unit einschalten.

## Daten und Dienstkonto

`KANLO_DATA_DIR` enthält `kanlo.sqlite3` mit WAL-Dateien. Das Verzeichnis wird
unter Unix mit 0700 angelegt, Backupdateien mit 0600. Die systemd-Vorlage setzt
zusätzlich UMask=0077, StateDirectory=kanlo und User/Group=kanlo. Die Regel in
`packaging/sysusers.d/kanlo.conf` richtet das nicht interaktiv nutzbare Konto ein.
Diese Vorlagen ersetzen keinen abgenommenen Installer.

`kanlo-server --check` führt eine SQLite-Integritätsprüfung aus. Daten nicht
durch Kopieren einer offenen Einzeldatei sichern: Die Web-/API-Datensicherung
liefert eine konsistente, unverschlüsselte JSON-Datei ohne Passcodehash,
API-Keyhash oder Sitzungstokens. Automatische Backups sind standardmäßig aus;
Intervall und Anzahl werden über Einstellungen gewählt. `KANLO_BACKUP_DIR`
überschreibt den Speicherort; Standard ist `KANLO_DATA_DIR/backups`.

`kanlo-server --migrate-data <leeres-zielverzeichnis>` erstellt zuerst ein Backup,
kopiert mit SQLite-Onlinebackup, prüft Identität/Inhalt/Integrität und sperrt den
alten Schreibstand gegen weitere Änderungen. Danach `KANLO_DATA_DIR` im Dienst
auf das neue Verzeichnis setzen. Das alte Verzeichnis bleibt erhalten. Die CLI
führt keinen systemd-/Mount-/SSD-Eingriff aus. Ein SSD-Gerätetest steht noch aus.

## Zwei Zugangswege

Standard: `KANLO_BIND=127.0.0.1:8080`. Für LAN beispielsweise
`KANLO_BIND=0.0.0.0:8080` zusammen mit `KANLO_ALLOWED_HOSTS=192.168.1.20:8080,kanlo.local:8080`.
Nur die tatsächliche Socket-Gegenstelle und der konfigurierte Router entscheiden
über den LAN-Zugang; Forwarded-Header erteilen keine Berechtigung.

Für externes HTTPS zuerst lokal einen gemeinsamen Passcode in den Einstellungen
setzen. Dann einen **separaten Loopback-Socket** wie
`KANLO_EXTERNAL_BIND=127.0.0.1:8081` und die genaue öffentliche Origin
`KANLO_PUBLIC_ORIGIN=https://kanlo.example` konfigurieren. Der TLS-Reverseproxy
muss ausschließlich diesen geschützten Socket bedienen und Host/Origin erhalten.
Er darf nie den LAN-Socket veröffentlichen. Kanlo richtet keinen Proxy und keine
Zertifikate automatisch ein.

Externe Browseraktionen brauchen Secure/HttpOnly/SameSite-Session, CSRF und
passende Origin. API-Clients verwenden eigene widerrufbare Bearer-Schlüssel.
Der gemeinsame Passcode gehört nicht in API-Aufrufe. Schlüssel nur einmal bei
Erstellung anzeigen; nicht in URLs, Logs oder Backups aufnehmen. Restore ändert
die Epoche und widerruft externe Sitzungen/Schlüssel; lokal gespeicherte
Operationen werden danach zur Prüfung angehalten.

## Offline und Wiederherstellung

Nur ausdrücklich besuchte Boards werden lokal gespeichert. Der Service Worker
speichert das App-Gerüst, keine authentifizierten API-Antworten in seinem Cache.
IndexedDB enthält Boarddaten und eine dauerhafte Operationsqueue, keine
Passcodes, API-Schlüssel, CSRF-Werte oder Sitzungstokens. Andere Geräte brauchen
vertrauenswürdiges HTTPS; gewöhnliches LAN-HTTP hat diese Browserfreigabe nicht.

Eine noch nicht übertragene Änderung bleibt bei Abbruch/Neuladen erhalten.
Konflikte und alte Epochen erfordern eine Entscheidung. Das ausdrückliche
Verwerfen entfernt die lokale Änderung und stellt den bestätigten Cachestand
wieder her. Vor dem Leeren der lokalen Daten zeigt die Oberfläche die Zahl
ausstehender Operationen. Ein noch nicht besuchtes Board kann offline nicht
geladen werden.
