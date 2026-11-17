# Kanlo 1.1.0-dev.2

Entwicklungs-Vorabversion nach Kanlo 1.0.0.

Geräte lassen sich vollständig aktualisieren. Der Updater ersetzt neben dem
Server auch die installierte TV-Anzeige, die `kanlo-tv.service`-Unit und sich
selbst und stellt bei einem Fehler alles gemeinsam wieder her. Geräte mit
1.0.x verwenden dafür einmalig den Updater aus dem neuen Paket; die Schritte
stehen in der Installationsanleitung.

Die Weboberfläche enthält einen Kalender für lokale Sprints und Meilensteine.
Ein Sprint kann einem Meilenstein angehören. Backlog-Tickets lassen sich einem
Sprint oder direkt einem Meilenstein zuordnen; die Karte zeigt die Namen an.
Datierte Meilensteine eines verbundenen Gitea-Repositorys erscheinen lesend im
Monatskalender und können als Vorlage für einen lokalen Sprint dienen. Die
Verbindung wird serverseitig über `KANLO_GITEA_URL`, `KANLO_GITEA_REPOSITORY`
und optional `KANLO_GITEA_TOKEN` eingerichtet.

Die TV-Anzeige bietet die Gestaltung „Hell“ oder „Farbig“. TV-Befehle werden
über eine eigene Fernbedienungsseite gesteuert, unabhängig vom geöffneten
Webticket. Eine Vorschau der Android-App „Kanlo TV“ öffnet diese Fernbedienung
direkt; sie liegt als Quellcode unter `android/` und ist nicht Teil der
Release-Anhänge.

Diese Vorabversion ersetzt keine Stable-Freigabe. Für den produktiven Einsatz
bleibt Kanlo 1.0.0 die stabile Version.
