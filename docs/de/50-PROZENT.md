# Messung des 50-Prozent-Ziels

Das Ziel aus dem Nutzerauftrag ist mindestens 35 von 70 ursprünglichen Aufgaben.
Die ursprünglichen Akzeptanzkriterien werden nicht geändert. Teilweise begonnene
Aufgaben zählen null; neue Hilfsaufgaben vergrößern den Zähler nicht.

Eine Aufgabe wird erst gezählt, wenn ihre implementierbaren Arbeitsschritte,
Akzeptanzkriterien und zugehörigen Tests belegt sind. Geräte-/Releaseaufgaben
bleiben ohne Geräte-/Releasebeleg offen. Ein bestandener Cross-Build ist kein
Pi-Laufzeittest. Alle Phasen bleiben bis zur Endabnahme als Entwicklungsversion
gekennzeichnet. Der Abschlussbericht führt jede gezählte KAN-ID samt Beleg auf.

Arbeitsreihenfolge: transaktionaler Domainkern und Recovery, sichere HTTP-Wege,
Preact/TypeScript-Webbedienung, persistente Offline-Queue und Konfliktdialoge,
Browser-/Mehrclient-/CI-Abnahme, danach Abgleich der tatsächlichen Aufgabenstände
in Gitea. GitHub bleibt der Codespiegel.

Zwischenstand: Backend-Erweiterung implementiert und lokal getestet; Web- und
Endabnahme laufen. Es wird noch kein erreichter Prozentwert behauptet.
