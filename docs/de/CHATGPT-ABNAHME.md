# ChatGPT-Abnahme

## Private Integration

Für die Abnahme eine Kanlo-Testinstanz mit zwei Boards und gleich benannten
Tickets verwenden. In ChatGPT auf dem Web das Plugin über **Plugins → + →
Add custom MCP server → Tunnel** verbinden. Der Tunnel muss dem Workspace
zugeordnet sein. Die erkannten Werkzeuge anzeigen lassen und ein neues
Gespräch mit dem Plugin öffnen. [OpenAI-Anleitung zur privaten Verbindung](https://developers.openai.com/plugins/deploy/connect-chatgpt)

| Test | Erwarteter Nachweis |
| --- | --- |
| „Liste meine Kanlo-Boards auf.“ | ChatGPT ruft das Plugin auf und zeigt die Boards aus der Testinstanz. |
| „Zeige das Ticket mit dem Titel WLAN-Test.“ | Bei identischen Titeln werden Board oder Ticketkennung abgefragt. |
| „Zeige IMG-1 für 2 Minuten auf dem TV.“ | Der konfigurierte Bildschirm zeigt das richtige Ticket. Der Test erfordert Kanlo 0.2.0-dev.8. |
| „Mache die letzte Änderung rückgängig.“ | Die Vorschau erscheint. Bis zur separaten Bestätigung bleiben die Daten gleich. |
| Gleiche Aktionen mit reinem Lesezugriff | Die Werkzeugliste enthält keine Schreib- oder Bestätigungsaktionen. |
| „Welche Einstellungskategorien gibt es?“ | Kategorie auswählen, bis zu fünf Namen abrufen und danach ausdrücklich Werte für ein Board anfordern. |
| GPT Actions mit exportiertem Leseschema | Schlüsselprüfung, Ticketsuche und Ticketabruf auf derselben Testinstanz. |

Die lokale MCP-Integration prüft Werkzeugaufrufe gegen einen isolierten
Kanlo-Server. Die Testreihe im tatsächlichen ChatGPT-Workspace steht aus.

## Diktat und Live-Voice

- Diktat schreibt gesprochenen Text in das Eingabefeld. Den erkannten Text
  vor dem Absenden prüfen. Anschließend im Textgespräch denselben Leseaufruf
  wie oben ausführen und den sichtbaren Werkzeugaufruf protokollieren.
- Für Live-Voice ein separates Gespräch starten. „Liste meine Kanlo-Boards
  auf“ sprechen und festhalten, ob das Plugin dort auswählbar ist und ein
  Werkzeugaufruf tatsächlich erscheint. Datum, App-Version, Plattform,
  Workspace und beobachteten Aufruf festhalten. Beide Tests sind offen.
  [OpenAI zu Diktat und Live-Voice](https://developers.openai.com/training/walkthroughs/using-voice)

## Öffentliche Einreichung

Die öffentliche Remote-Variante erfordert eine verifizierte Entwickleridentität,
einen stabilen öffentlichen HTTPS-Endpunkt und die Prüfung der Domain.
Das Portal bietet keinen Eingabedialog für einen Bearer-Token oder API-Schlüssel.
Für eine geschützte öffentliche Variante ist deshalb ein freigegebener OAuth-Weg
oder eine mit OpenAI abgestimmte Alternative erforderlich. Der private
Kanlo-Tunnel eignet sich für den Workspace-Test. Für eine öffentliche
Einreichung sind außerdem Lizenz- und Markenfreigabe erforderlich.
[OpenAI-Anforderungen zur Einreichung](https://developers.openai.com/plugins/deploy/submission)
