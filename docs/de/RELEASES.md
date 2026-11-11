# Tag-gesteuerte Releases

Ein Versions-Tag startet `.github/workflows/ci.yml` auf Gitea. GitHub spiegelt
Quellcode und Tags; GitHub Actions bleiben deaktiviert. Die Pipeline veröffentlicht
die neuen Release-Downloads auf Gitea. Tags folgen exakt der Cargo-Version:

- `vX.Y.Z-dev.N`: Entwicklungs-Vorabversion.
- `vX.Y.Z-beta.N` oder `vX.Y.Z-rc.N`: weiterhin als Prerelease markiert.
- `vX.Y.Z`: Stable, nur mit vollständigen Belegen in `release/readiness.json`.

Der erste Release ist `v0.1.0-dev.1`. Er enthält Server/Web, keine native
TV-Laufzeit, keinen Kiosk und kein Flash-Image. Die Geräte-Images beginnen mit
`v0.2.0-dev.1`. Die Repository-Sichtbarkeit
bleibt privat; die ausstehende Lizenzfreigabe wird nicht ersetzt.

## Ablauf

1. Codeprüfung, Rust-Tests, TypeScript/Webbuild, Abhängigkeitsprüfung und
   Release-Werkzeugtests. Desktop und Mobilgerät erhalten jeweils einen eigenen
   Testserverstart, damit das produktive Rate Limit aktiv bleibt.
2. Debian-13-Debug-/Release-Builds für AMD64, ARM64 und armhf. Die Plattform-
   Pakete enthalten den Server, eingebettete Webassets, Versions-/Commitdaten,
   Lizenzhinweise, Unit-/Dienstkonto-Vorlagen und DE-/EN-Installationsanleitungen.
3. Der Headless-Job lädt das hochgeladene AMD64-Paket herunter und startet die
   enthaltene Release-Binärdatei ohne Netzwerk/Display, als unprivilegierter
   Nutzer. Version, Neustart, SIGTERM mit SSE und Datenintegrität werden geprüft.
   Zusätzlich baut die Image-Matrix vier Geräte-Images und bootet pro Variante
   eine getrennte Testkopie zweimal unter QEMU: Erstkonfiguration, Expansion,
   Dienste, Dateirechte, ein geschriebenes Ticket nach Reboot und SQLite-
   Integrität müssen erfolgreich sein. Native SDL/Wayland-Ausgabe wird getrennt
   ohne Netzwerk auf einem headless Compositor geprüft. Hardwareabnahme bleibt
   separat. Details: [Geräte-Images](../../release/IMAGES-DE.md).
4. Erst nach allen erfolgreichen Jobs erzeugt die Veröffentlichung ein
   Quellarchiv, Manifest und `SHA256SUMS`, prüft alle Plattformpakete und lädt
   die 14 Anhänge zunächst in einen Entwurf: drei Serverpakete, vier Images,
   vier Image-Nachweise mit OS-Paketlisten, Quellarchiv, Manifest und Prüfsummen.
5. Jeder Upload wird erneut heruntergeladen und verglichen. Erst dann wird der
   Entwurf veröffentlicht. Ein Fehler lässt ihn unveröffentlicht. Wiederholung
   ergänzt passende Anhänge; abweichende vorhandene Dateien werden abgewiesen.

Gitea Actions stellt das Job-Token bereit. Schreibrechte auf
Repository-Inhalte besitzt nur der Tag-Veröffentlichungsjob. Es werden keine
neuen dauerhaften Tokens im Repository oder auf dem Entwicklungsrechner abgelegt.
Die Gitea-Pipeline benötigt keinen Zugriff auf GitHub-Secrets.

## Bedienung

Cargo-Version, `release/NOTES.md` und Changelog müssen zum geplanten Tag passen.
Nach grüner Branch-/PR-Abnahme den unveränderlichen Versions-Tag setzen und
beide Remotes abgleichen:

```sh
git tag -a v0.2.0-dev.1 <geprüfter-commit> -m 'Kanlo 0.2.0-dev.1'
git push origin refs/tags/v0.2.0-dev.1
git push github refs/tags/v0.2.0-dev.1
```

Tags nicht verschieben oder erzwingen. Für eine neue Programmversion einen neuen
Versions-Tag verwenden. Einen wegen Netzwerkproblemen abgebrochenen Lauf am
gleichen unveränderten Tag erneut starten. Ein veröffentlichter Release wird
nicht automatisch mit anderen Dateien überschrieben.

Stable wird verweigert, solange native TV-, Geräte-, Backup/Update/Rollback-,
Security-, Dokumentations-, Lizenz- und Markenbelege für genau diesen Commit
fehlen. Ein Dev-Tag kann diese Freigaben nicht als erfüllt markieren.

## Quellen der Schnittstellen

- [Gitea-Job-Tokens und Rechte](https://docs.gitea.com/usage/actions/token-permissions/)
- [Gitea-Actions und Workflow-Suche](https://docs.gitea.com/administration/config-cheat-sheet/#actions-actions)
