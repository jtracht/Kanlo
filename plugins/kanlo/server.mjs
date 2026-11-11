import { randomUUID } from "node:crypto";
import { readFileSync } from "node:fs";
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";

const origin = URL.parse(process.env.KANLO_URL ?? "http://127.0.0.1:8080");
if (!origin) throw new Error("KANLO_URL ist ungültig.");
const local = ["localhost", "127.0.0.1", "[::1]"].includes(origin.hostname);
const apiKey = process.env.KANLO_API_KEY;
const writesEnabled = process.env.KANLO_ENABLE_WRITES === "1";
const clientId = randomUUID();
const { version } = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf8"));

if (origin.username || origin.password || origin.pathname !== "/" || origin.search || origin.hash ||
    !(origin.protocol === "https:" || (local && origin.protocol === "http:"))) {
  throw new Error("KANLO_URL muss eine HTTPS-Origin oder eine lokale HTTP-Origin sein.");
}
if (!local && !apiKey) {
  throw new Error("Für eine entfernte Kanlo-Instanz ist KANLO_API_KEY erforderlich.");
}

async function request(path, body) {
  const headers = { Accept: "application/json" };
  if (apiKey) headers.Authorization = `Bearer ${apiKey}`;
  if (body !== undefined) {
    headers["Content-Type"] = "application/json";
    headers.Origin = origin.origin;
  }
  const response = await fetch(new URL(`/api/v1/${path}`, origin), {
    method: body === undefined ? "GET" : "POST",
    headers,
    body: JSON.stringify(body),
    redirect: "error",
    signal: AbortSignal.timeout(10_000),
  });
  if (!response.ok) {
    await response.body?.cancel();
    throw new Error(`Kanlo-API antwortet mit HTTP ${response.status}.`);
  }
  return response.json();
}

async function mutate(command) {
  const { instance_id, epoch } = await request("health");
  return request("commands", {
    op_id: randomUUID(),
    client_id: clientId,
    server_instance_id: instance_id,
    base_epoch: epoch,
    command,
  });
}

const server = new McpServer({ name: "kanlo", version });
const id = z.string().regex(/^[A-Za-z0-9_-]{1,128}$/);
const paging = {
  offset: z.number().int().min(0).default(0),
  limit: z.number().int().min(1).max(25).default(10),
};

function page(items, { offset, limit }, project) {
  const next = offset + limit;
  return {
    items: items.slice(offset, next).map(project),
    next_offset: next < items.length ? next : null,
  };
}

function tool(name, description, inputSchema, execute, { readOnlyHint = true, destructiveHint = false } = {}) {
  server.registerTool(name, {
    description,
    inputSchema,
    annotations: { readOnlyHint, destructiveHint, idempotentHint: false, openWorldHint: false },
  }, async (input) => {
    try {
      const result = await execute(input);
      return { content: [{ type: "text", text: JSON.stringify(result) }], structuredContent: result };
    } catch (error) {
      const message = error.message.startsWith("Kanlo-API antwortet mit HTTP ")
        ? error.message
        : "Kanlo konnte nicht erreicht oder die Antwort nicht gelesen werden.";
      return { isError: true, content: [{ type: "text", text: message }] };
    }
  });
}

tool("kanlo_list_boards", "Kanlo-Boards mit IDs und Ticketpräfixen auflisten.", paging,
  async (input) => page(await request("boards"), input, ({ id, name, prefix }) => ({ id, name, prefix })));

tool("kanlo_list_columns", "Spalten eines Kanlo-Boards auflisten.", { board_id: id, ...paging },
  async (input) => page(await request(`boards/${input.board_id}/columns`), input, (column) => column));

tool("kanlo_list_tickets", "Kanlo-Tickets durchsuchen. Ohne Board-ID wird boardübergreifend gesucht. Optional nach Spalte, Text oder Status filtern.", {
  board_id: id.optional(),
  column_id: id.optional(),
  query: z.string().trim().min(1).max(200).optional(),
  status: z.enum(["active", "open", "done", "blocked", "backlog", "overdue", "archive", "trash", "all"]).optional(),
  ...paging,
}, async (input) => request(`ticket-search?${new URLSearchParams(input)}`));

tool("kanlo_get_ticket", "Ein Ticket per interner ID oder sichtbarer Ticketkennung abrufen.", {
  ticket_id: id,
}, async ({ ticket_id }) => ({ ticket: await request(`tickets/${ticket_id}`) }));

tool("kanlo_list_comments", "Kommentare zu einem Kanlo-Ticket seitenweise abrufen.", {
  ticket_id: id,
  ...paging,
}, async ({ ticket_id, ...page }) => request(`tickets/${ticket_id}/comments?${new URLSearchParams(page)}`));

if (writesEnabled) {
  tool("kanlo_create_ticket", "Ein Ticket in Kanlo anlegen. Nur auf ausdrücklichen Nutzerwunsch ausführen.", {
    board_id: id,
    title: z.string().trim().min(1).max(200),
    description: z.string().max(20_000).default(""),
    column_id: id.optional(),
  }, async ({ board_id, title, description, column_id }) => {
    const receipt = await mutate({
      kind: "create_ticket",
      board_id,
      title,
      description,
      importance: 0,
      column_id: column_id ?? null,
      parent_id: null,
      type_id: null,
    });
    return { id: receipt.result.entity_id, revision: receipt.revision };
  }, { readOnlyHint: false });

  tool("kanlo_update_ticket", "Titel, Beschreibung oder Wichtigkeit auf Nutzerwunsch ändern. Die Revision aus der letzten Abfrage angeben. Bei Konflikt erneut lesen und die Änderung mit dem Nutzer abgleichen.", {
    ticket_id: id,
    revision: z.number().int().nonnegative(),
    fields: z.strictObject({
      title: z.string().trim().min(1).max(300).optional(),
      description: z.string().max(20_000).optional(),
      importance: z.number().int().min(0).max(255).optional(),
    }).refine((fields) => Object.keys(fields).length > 0, "Mindestens ein Feld ist erforderlich."),
  }, async ({ ticket_id, revision, fields }) => {
    const receipt = await mutate({ kind: "patch_ticket", id: ticket_id, base_revision: revision, fields });
    return { id: receipt.result.entity_id, revision: receipt.revision };
  }, { readOnlyHint: false });

  tool("kanlo_add_comment", "Auf ausdrücklichen Nutzerwunsch einen Markdown-Kommentar zu einem Ticket hinzufügen.", {
    ticket_id: id,
    markdown: z.string().trim().min(1).max(4_000),
  }, async ({ ticket_id, markdown }) => {
    const receipt = await mutate({ kind: "add_comment", ticket_id, markdown });
    return { id: receipt.result.entity_id, revision: receipt.revision };
  }, { readOnlyHint: false });

  tool("kanlo_move_ticket", "Ein Ticket auf Nutzerwunsch in eine Zielspalte oder in das Backlog verschieben. Kanlo kann dabei Status und Untertickets mitändern. Zielboard und aktuelle Ticketrevision angeben. Bei Konflikt erneut lesen und mit dem Nutzer abgleichen.", {
    ticket_id: id,
    revision: z.number().int().nonnegative(),
    board_id: id,
    column_id: id.nullable().describe("ID der Zielspalte oder null für das Backlog."),
  }, async ({ ticket_id, revision, board_id, column_id }) => {
    const receipt = await mutate({
      kind: "move_ticket",
      id: ticket_id,
      base_revision: revision,
      board_id,
      column_id,
      activate_types: false,
      convert_types: {},
    });
    return { id: receipt.result.entity_id, revision: receipt.revision };
  }, { readOnlyHint: false, destructiveHint: true });
}

await server.connect(new StdioServerTransport());
