import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

const executable = fileURLToPath(new URL(
  `../../target/debug/kanlo-server${process.platform === "win32" ? ".exe" : ""}`,
  import.meta.url,
));

test("MCP tools read and create tickets through an isolated Kanlo instance", { timeout: 30_000 }, async (t) => {
  const data = await mkdtemp(join(tmpdir(), "kanlo-mcp-"));
  const env = { ...process.env, KANLO_DATA_DIR: data, KANLO_BIND: "127.0.0.1:0" };
  delete env.KANLO_EXTERNAL_BIND;
  const kanlo = spawn(executable, [], {
    env,
    stdio: ["ignore", "pipe", "inherit"],
  });
  const closed = once(kanlo, "close").catch(() => {});
  t.after(async () => {
    kanlo.kill();
    await closed;
    await rm(data, { recursive: true, force: true, maxRetries: 3 });
  });

  const lines = createInterface({ input: kanlo.stdout });
  const [line] = await Promise.race([
    once(lines, "line", { signal: AbortSignal.timeout(5_000) }),
    closed.then(() => { throw new Error("Kanlo exited before opening its local port."); }),
  ]);
  const url = line.match(/http:\/\/127\.0\.0\.1:\d+/)?.[0];
  assert.ok(url, line);

  async function connect(writes) {
    const transport = new StdioClientTransport({
      command: process.execPath,
      args: [fileURLToPath(new URL("./server.mjs", import.meta.url))],
      env: { KANLO_URL: url, KANLO_ENABLE_WRITES: writes ? "1" : "0", KANLO_API_KEY: "" },
    });
    const client = new Client({ name: "kanlo-test", version: "1.0.0" });
    t.after(() => client.close());
    await client.connect(transport);
    return client;
  }

  const reader = await connect(false);
  const { tools } = await reader.listTools();
  assert.equal(tools.length, 4);
  assert.ok(tools.every((tool) => tool.annotations.readOnlyHint));
  const boards = await reader.callTool({ name: "kanlo_list_boards" });
  assert.equal(boards.isError, undefined, JSON.stringify(boards));
  const board_id = boards.structuredContent.items[0].id;
  const columns = await reader.callTool({ name: "kanlo_list_columns", arguments: { board_id } });
  assert.equal(columns.isError, undefined, JSON.stringify(columns));
  const column_id = columns.structuredContent.items[0].id;
  const writer = await connect(true);
  const title = "MCP integration ticket";
  const result = await writer.callTool({
    name: "kanlo_create_ticket", arguments: { board_id, column_id, title },
  });
  assert.equal(result.isError, undefined, JSON.stringify(result));
  const tickets = await reader.callTool({
    name: "kanlo_list_tickets", arguments: { board_id, column_id, query: "integration", limit: 1 },
  });
  const ticket = tickets.structuredContent.items[0];
  assert.equal(ticket.id, result.structuredContent.id);
  assert.equal(tickets.structuredContent.next_offset, null);
  const detail = await reader.callTool({
    name: "kanlo_get_ticket", arguments: { ticket_id: ticket.public_id },
  });
  assert.equal(detail.structuredContent.ticket.title, title);
  assert.equal(detail.structuredContent.ticket.column_id, column_id);
});
