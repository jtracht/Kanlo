import { useState } from "preact/hooks";
import { Message, useT } from "./ui";

const bearer = [{ kanloKey: [] }];
const ok = { "200": { description: "Success", content: { "application/json": {
  schema: { type: "object", additionalProperties: true },
} } } };

function makeActionsSchema(origin: string) {
  return {
    openapi: "3.1.0",
    info: { title: "Kanlo", description: "Read tickets from your Kanlo instance.", version: "1.0.0" },
    servers: [{ url: origin }],
    components: { securitySchemes: { kanloKey: { type: "http", scheme: "bearer" } } },
    security: bearer,
    paths: {
      "/api/v1/integrations/verify": { get: {
        operationId: "verifyKanloConnection", summary: "Verify the Kanlo API key", responses: ok,
      } },
      "/api/v1/ticket-search": { get: {
        operationId: "searchKanloTickets", summary: "Find tickets across Kanlo boards",
        parameters: [
          { name: "query", in: "query", schema: { type: "string" } },
          { name: "board_id", in: "query", schema: { type: "string" } },
          { name: "status", in: "query", schema: { type: "string", enum: ["active", "open", "done", "blocked", "backlog", "overdue", "archive", "trash", "all"] } },
          { name: "offset", in: "query", schema: { type: "integer", minimum: 0 } },
          { name: "limit", in: "query", schema: { type: "integer", minimum: 1, maximum: 25, default: 5 } },
        ],
        responses: ok,
      } },
      "/api/v1/tickets/{id}": { get: {
        operationId: "getKanloTicket", summary: "Get a ticket by visible ID or internal ID",
        parameters: [{ name: "id", in: "path", required: true, schema: { type: "string" } }],
        responses: ok,
      } },
    },
  };
}

export function IntegrationSetup() {
  const t = useT();
  const [path, setPath] = useState<"mcp" | "actions">("mcp");
  const [origin, setOrigin] = useState(location.protocol === "https:" ? location.origin : "");
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState(false);
  const secureKeyTest = location.protocol === "https:" ||
    ["localhost", "127.0.0.1", "[::1]"].includes(location.hostname);

  async function verify() {
    const token = key.trim();
    setKey("");
    setBusy(true);
    setMessage("");
    try {
      const response = await fetch("/api/v1/integrations/verify", {
        headers: { Authorization: `Bearer ${token}` },
        credentials: "omit",
        cache: "no-store",
        redirect: "error",
        signal: AbortSignal.timeout(10000),
      });
      setError(!response.ok);
      setMessage(response.ok
        ? t("Kanlo hat diesen API-Schlüssel akzeptiert.", "Kanlo accepted this API key.")
        : t("Kanlo hat den Schlüssel nicht akzeptiert.", "Kanlo did not accept this key."));
    } catch {
      setError(true);
      setMessage(t("Kanlo war für den Test nicht erreichbar.", "Kanlo could not be reached for this test."));
    } finally {
      setBusy(false);
    }
  }

  function downloadSchema() {
    try {
      const url = new URL(origin.trim());
      if (url.protocol !== "https:" || url.username || url.password || url.pathname !== "/" || url.search || url.hash) {
        throw new Error();
      }
      const blob = new Blob([JSON.stringify(makeActionsSchema(url.origin), null, 2)], { type: "application/json" });
      const href = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = href;
      anchor.download = "kanlo-actions.openapi.json";
      anchor.click();
      setTimeout(() => URL.revokeObjectURL(href), 1000);
    } catch {
      setError(true);
      setMessage(t("Gib die geschützte HTTPS-Adresse ohne Pfad ein.", "Enter the protected HTTPS address without a path."));
    }
  }

  return <details>
    <summary>{t("Integration einrichten", "Set up integration")}</summary>
    <label>{t("Verbindungsart", "Connection type")}
      <select value={path} onChange={(event) => {
        setPath(event.currentTarget.value === "mcp" ? "mcp" : "actions");
        setKey("");
        setMessage("");
      }}>
        <option value="mcp">MCP</option>
        <option value="actions">GPT Actions</option>
      </select>
    </label>
    <ol>
      <li>{t("Erstelle oben einen separaten Kanlo-API-Schlüssel.", "Create a separate Kanlo API key above.")}</li>
      {path === "mcp" ? <>
        <li>{t("Entpacke das passende Kanlo-Plugin und installiere seine Abhängigkeiten mit npm ci.", "Extract the Kanlo plugin and install its dependencies with npm ci.")}</li>
        <li>{t("Setze KANLO_API_KEY beim Plugin-Prozess. Starte Claude Code mit --plugin-dir oder registriere den privaten Tunnel in ChatGPT.", "Set KANLO_API_KEY for the plugin process. Start Claude Code with --plugin-dir or register the private tunnel in ChatGPT.")}</li>
        <li>{t("Lass dir im verbundenen Chat die Kanlo-Boards auflisten.", "In the connected chat, ask to list the Kanlo boards.")}</li>
      </> : <>
        <li>{t("Gib die geschützte öffentliche HTTPS-Adresse ein und lade das Leseschema herunter.", "Enter the protected public HTTPS address and download the read-only schema.")}</li>
        <li>{t("Importiere es im GPT-Editor unter Actions und hinterlege den Schlüssel als API-Key mit Bearer-Authentifizierung.", "Import it under Actions in the GPT editor and configure the key with Bearer authentication.")}</li>
        <li>{t("Teste verifyKanloConnection im GPT-Editor.", "Test verifyKanloConnection in the GPT editor.")}</li>
      </>}
    </ol>
    {path === "actions" && <div class="input-row">
      <input type="url" aria-label={t("Geschützte HTTPS-Adresse", "Protected HTTPS address")}
        value={origin} onInput={(event) => setOrigin(event.currentTarget.value)} placeholder="https://kanlo.example.org" />
      <button type="button" class="secondary" onClick={downloadSchema}>{t("Leseschema herunterladen", "Download read schema")}</button>
    </div>}
    <div class="input-row">
      <input type="password" autoComplete="off" disabled={!secureKeyTest} aria-label={t("API-Schlüssel zum Testen", "API key to test")}
        value={key} onInput={(event) => setKey(event.currentTarget.value)} />
      <button type="button" class="secondary" disabled={!secureKeyTest || !key.trim() || busy} onClick={() => void verify()}>
        {t("Schlüssel an diesem Kanlo prüfen", "Test key on this Kanlo instance")}
      </button>
    </div>
    {!secureKeyTest && <Message>{t("Öffne Kanlo auf demselben Rechner oder über HTTPS, um einen Schlüssel zu testen.", "Open Kanlo on the same machine or over HTTPS to test a key.")}</Message>}
    {message && <Message error={error}>{message}</Message>}
  </details>;
}
