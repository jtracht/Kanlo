# Kanlo für Claude Code und ChatGPT

Die gemeinsame MCP-Anbindung fragt Boards, Spalten und aktive Tickets ab.
Tickets lassen sich nach Titel, sichtbarer Kennung und Spalte filtern.
Die Ticketdetails können über die sichtbare Kennung geöffnet werden.
Mit freigegebenem Schreibzugriff können beide Clients auch Tickets anlegen.

## Vorbereitung

Im Stammverzeichnis des Kanlo-Repositories mit Node.js ab Version 22 ausführen.

```sh
npm ci --prefix plugins/kanlo --omit=dev
```

Der MCP-Prozess verwendet die folgenden Umgebungsvariablen.

| Variable | Einstellung |
| --- | --- |
| `KANLO_URL` | Standard `http://127.0.0.1:8080`. Für eine entfernte Instanz die geschützte HTTPS-Origin ohne Pfad verwenden. |
| `KANLO_API_KEY` | Separater widerrufbarer Kanlo-API-Schlüssel. Für entfernte Instanzen erforderlich. Siehe [Betrieb](../../docs/de/BETRIEB.md). |
| `KANLO_ENABLE_WRITES` | `1` stellt das Werkzeug zum Anlegen von Tickets bereit. Standardmäßig werden nur Lesewerkzeuge angeboten. |

Die Variablen vor dem Start von Claude Code beziehungsweise des Tunnel-Clients
in dessen Prozessumgebung setzen. Schlüssel außerhalb des Repositories verwalten.

## Claude Code

```sh
claude --plugin-dir ./plugins/kanlo
```

Mit `/mcp` den Verbindungsstatus prüfen und Claude danach bitten, die Kanlo-Boards
aufzulisten. Claude Code startet den MCP-Prozess aus dem Plugin-Verzeichnis.
[Claude Code Plugin-Referenz](https://code.claude.com/docs/en/plugins-reference)

## ChatGPT

ChatGPT erstellt das private Plugin bei der Registrierung des MCP-Servers.
Der Workspace muss das Hinzufügen eigener MCP-Server und die Tunnel-Nutzung
zulassen. Der Tunnel-Client kann auf dem Kanlo-Host laufen und die Standardadresse
verwenden. Läuft er auf einem anderen Rechner, benötigt er den geschützten
HTTPS-Zugang zu Kanlo.

1. Einen privaten Tunnel erstellen und den Tunnel-Client einrichten.
   Das stdio-Profil startet dieselbe MCP-Anbindung wie Claude Code. Für
   `--tunnel-id` die ID des eigenen Tunnels einsetzen und den Skriptpfad anpassen.

   ```sh
   tunnel-client init --sample sample_mcp_stdio_local --profile kanlo \
     --tunnel-id TUNNEL_ID --mcp-command "node /absoluter/pfad/plugins/kanlo/server.mjs"
   tunnel-client doctor --profile kanlo --explain
   tunnel-client run --profile kanlo
   ```

   [OpenAI Secure MCP Tunnel](https://developers.openai.com/api/docs/guides/secure-mcp-tunnels)

2. In [ChatGPT Plugins](https://chatgpt.com/plugins) einen eigenen MCP-Server
   hinzufügen, als Namen **Kanlo** setzen und unter **Connection** den
   privaten **Tunnel** auswählen. Für die private Tunnel-Verbindung die
   Option **No authentication** für den MCP-Prozess wählen und **Create as a
   plugin** auswählen. Der Kanlo-API-Schlüssel bleibt beim MCP-Prozess.
   [OpenAI-Anleitung zur Registrierung](https://developers.openai.com/api/docs/guides/custom-mcp-server)
3. Das Plugin in einem neuen Chat auswählen und die Boards auflisten lassen.
   Die freigegebenen Personen im ChatGPT-Workspace greifen über dieselbe
   konfigurierte Kanlo-Verbindung zu. Die Freigabe auf Personen mit
   entsprechendem Kanlo-Zugriff beschränken.

## Integrationstest

```sh
cargo build --locked -p kanlo-api --bin kanlo-server
npm test --prefix plugins/kanlo
```

Der Test startet einen lokalen Kanlo-Server mit temporärer Datenbank und prüft
die MCP-Aufrufe über stdio einschließlich einer Ticketanlage.
