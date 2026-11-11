import { randomUUID } from "node:crypto";
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";

const origin = URL.parse(process.env.KANLO_URL ?? "http://127.0.0.1:8080");
if (!origin) throw new Error("KANLO_URL ist ungültig.");
const local = ["localhost", "127.0.0.1", "[::1]"].includes(origin.hostname);
const apiKey = process.env.KANLO_API_KEY;
const writesEnabled = process.env.KANLO_ENABLE_WRITES === "1";
const clientId = randomUUID();

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

const server = new McpServer({ name: "kanlo", version: "0.1.0" });
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

function ticketSummary({ id, public_id, board_id, column_id, title, importance, done, revision }) {
  return { id, public_id, board_id, column_id, title, importance, done, revision };
}

function tool(name, description, inputSchema, execute, readOnlyHint = true) {
  server.registerTool(name, {
    description,
    inputSchema,
    annotations: { readOnlyHint, destructiveHint: false, idempotentHint: false, openWorldHint: false },
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

tool("kanlo_list_tickets", "Tickets eines Boards auflisten. Optional nach Spalte und Text im Titel oder der Ticketkennung filtern.", {
  board_id: id,
  column_id: id.optional(),
  query: z.string().trim().min(1).max(200).optional(),
  ...paging,
}, async (input) => {
  const tickets = await request(`boards/${input.board_id}/tickets`);
  const query = input.query?.toLowerCase();
  const matches = tickets.filter((ticket) =>
    (!input.column_id || ticket.column_id === input.column_id) &&
    (!query || ticket.title.toLowerCase().includes(query) || ticket.public_id.toLowerCase().includes(query)));
  return page(matches, input, ticketSummary);
});

tool("kanlo_get_ticket", "Ein Ticket per interner ID oder sichtbarer Ticketkennung abrufen.", {
  ticket_id: id,
}, async ({ ticket_id }) => ({ ticket: await request(`tickets/${ticket_id}`) }));

if (writesEnabled) {
  tool("kanlo_create_ticket", "Ein Ticket in Kanlo anlegen. Nur auf ausdrücklichen Nutzerwunsch ausführen.", {
    board_id: id,
    title: z.string().trim().min(1).max(200),
    description: z.string().max(20_000).default(""),
    column_id: id.optional(),
  }, async ({ board_id, title, description, column_id }) => {
    const { instance_id, epoch } = await request("health");
    const receipt = await request("commands", {
      op_id: randomUUID(),
      client_id: clientId,
      server_instance_id: instance_id,
      base_epoch: epoch,
      command: {
        kind: "create_ticket",
        board_id,
        title,
        description,
        importance: 0,
        column_id: column_id ?? null,
        parent_id: null,
        type_id: null,
      },
    });
    return { id: receipt.result.entity_id, revision: receipt.revision };
  }, false);
}

await server.connect(new StdioServerTransport());
