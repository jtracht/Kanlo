# Gitea und GitHub

Angelegte private Hauptquelle: `https://git.tracht-digital.de/jtracht/Kanlo`.
Angelegter privater Codespiegel: `https://github.com/jtracht/Kanlo`.
Elf Meilensteine und 70 offene Aufgaben liegen in Gitea, mit echten Nummern
unter `docs/planning/remote-state.json`. Push und CI werden separat geprüft.

Gitea-Connector verbunden und für Repository-/Meilenstein-/Issue-Erstellung genutzt.
Die konkreten Aufrufe wurden ohne zusätzlichen Freigabeschritt erfolgreich ausgeführt.
Er unterstützt keinen vollständigen Git-Push,
keine Repository-Push-Mirror-Konfiguration und keine Mehrdateien-Commits.
Für Git-Transport wird deshalb ergänzend lokales Git eingesetzt, soweit eine
bereits eingerichtete Git-Authentifizierung vorhanden ist. Keine Connector-
Tokens auslesen oder Freigaben über alternative Zugangswege umgehen.

## Geplanter Pushablauf

`origin` zeigt zum Gitea-Repository. `github` zeigt zum GitHub-Spiegel.
`scripts/push-mirrors.ps1` überträgt zuerst den ausgewählten Branch auf Gitea,
dann auf GitHub und prüft die Commit-IDs. Fehler werden nicht als Erfolg
ausgegeben. Es gibt keinen Force-Push und keine automatische Ref-Löschung.

Das ist eine Codespiegelung beim expliziten Push aus diesem Checkout, kein
serverseitiger Dauersync. Issues, Meilensteine, PRs und Releases sind dadurch
nicht gespiegelt. Ein späterer serverseitiger Gitea-Push-Mirror braucht eine
geeignete freigegebene Transport-/Secret-Konfiguration. Keine Secrets im Repo.

## Branches und Veröffentlichungen

Nach dem initialen Import Featurebranches mit Review verwenden. Schutzregeln
erst anhand tatsächlicher CI-Namen aktivieren, um den Erstimport nicht zu sperren.
Keine Stable-Tags/öffentlichen Repositories vor vollständiger Abnahme und
Lizenzprüfung. Entwicklungsartefakte im CI sind keine Stable-Releases.
