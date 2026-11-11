# Kanlo mit Claude Code und ChatGPT verbinden

Öffne **Einstellungen → Zugriff und Integrationen**. Lege für jede Verbindung
einen eigenen Schlüssel an. Prüfe ihn im Assistenten unter **Integration
einrichten** auf localhost oder über HTTPS. Der Test sendet den Schlüssel
ausschließlich an die geöffnete Kanlo-Instanz und leert das Eingabefeld danach.
Widerrufe einen nicht mehr benötigten Schlüssel in denselben Einstellungen.

## MCP

1. Entpacke das Quellpaket für den gewünschten Client. Führe mit Node.js ab
   Version 22 `npm ci --omit=dev` im Paketordner aus.
2. Setze `KANLO_API_KEY` in der Prozessumgebung. Auf dem Kanlo-Host ist
   `http://127.0.0.1:8080` voreingestellt. Für einen anderen Host setze
   `KANLO_URL` auf die geschützte HTTPS-Adresse ohne Pfad.
3. Starte [Claude Code](../../release/plugins/CLAUDE-CODE.md) mit dem entpackten
   Plugin-Verzeichnis oder richte [ChatGPT](../../release/plugins/CHATGPT.md)
   mit einem privaten MCP-Tunnel ein.
4. Lass dir im verbundenen Chat die Kanlo-Boards auflisten. Aktiviere
   `KANLO_ENABLE_WRITES=1` nur für Verbindungen, die Tickets oder die Anzeige
   verändern dürfen.

## GPT Actions

1. Richte den [geschützten HTTPS-Zugang](BETRIEB.md#zwei-zugangswege) ein.
   GPT Actions benötigt eine von ChatGPT erreichbare HTTPS-Adresse.
2. Wähle **GPT Actions** im Assistenten, gib diese Adresse ohne Pfad ein und
   lade das OpenAPI-Leseschema herunter.
3. Importiere das Schema im Actions-Bereich des GPT-Editors. Wähle
   **API Key → Bearer** und trage den eigenen Kanlo-Schlüssel in die
   Authentifizierungseinstellungen ein.
4. Teste **verifyKanloConnection** im Editor. Danach kannst du Tickets über
   **searchKanloTickets** suchen und über **getKanloTicket** abrufen.
   [OpenAI-Anleitung](https://developers.openai.com/api/docs/actions/getting-started)

## English

1. Open **Settings → Access and integrations → Set up integration**. Create
   a separate API key for each connection and test it from loopback or HTTPS.
2. **MCP**. Extract the source package, run `npm ci --omit=dev`, and set
   `KANLO_API_KEY` in the MCP process environment. Set `KANLO_URL` to the
   protected HTTPS origin when Kanlo runs on a different machine. Start
   Claude Code with the plugin directory or register ChatGPT through a private
   tunnel. Ask the connected client to list your boards.
3. **GPT Actions**. Set up a protected HTTPS endpoint reachable by ChatGPT.
   Select GPT Actions in the wizard and download the generated read schema
   for that origin. Import it in the GPT editor, set authentication to
   **API Key → Bearer**, and test **verifyKanloConnection** in the editor.
   [OpenAI setup guide](https://developers.openai.com/api/docs/actions/getting-started)
