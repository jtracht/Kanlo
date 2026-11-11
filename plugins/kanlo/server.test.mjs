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

test("MCP tools manage tickets through an isolated Kanlo instance", { timeout: 30_000 }, async (t) => {
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
  assert.equal(tools.length, 5);
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
  const update = {
    ticket_id: ticket.public_id,
    revision: detail.structuredContent.ticket.revision,
    fields: { description: "Searchable detail", importance: 40 },
  };
  const updated = await writer.callTool({ name: "kanlo_update_ticket", arguments: update });
  assert.equal(updated.isError, undefined, JSON.stringify(updated));
  const stale = await writer.callTool({ name: "kanlo_update_ticket", arguments: update });
  assert.equal(stale.isError, true);
  assert.match(stale.content[0].text, /409/);
  const found = await reader.callTool({
    name: "kanlo_list_tickets", arguments: { query: "Searchable detail", status: "open" },
  });
  assert.equal(found.structuredContent.items[0].id, ticket.id);
  assert.equal(found.structuredContent.items[0].importance, 40);
  const comment = await writer.callTool({
    name: "kanlo_add_comment", arguments: { ticket_id: ticket.public_id, markdown: "Ein Testkommentar." },
  });
  assert.equal(comment.isError, undefined, JSON.stringify(comment));
  assert.ok(comment.structuredContent.id);
  const secondComment = await writer.callTool({
    name: "kanlo_add_comment", arguments: { ticket_id: ticket.public_id, markdown: "Zweiter Kommentar." },
  });
  assert.equal(secondComment.isError, undefined, JSON.stringify(secondComment));
  const comments = await reader.callTool({
    name: "kanlo_list_comments", arguments: { ticket_id: ticket.public_id, limit: 1 },
  });
  assert.equal(comments.structuredContent.next_offset, 1);
  const moreComments = await reader.callTool({
    name: "kanlo_list_comments", arguments: { ticket_id: ticket.id, limit: 1, offset: 1 },
  });
  assert.equal(moreComments.structuredContent.next_offset, null);
  const [first] = comments.structuredContent.items;
  const [second] = moreComments.structuredContent.items;
  assert.deepEqual(new Map([[first.id, first.markdown], [second.id, second.markdown]]),
    new Map([[comment.structuredContent.id, "Ein Testkommentar."], [secondComment.structuredContent.id, "Zweiter Kommentar."]]));
  assert.ok(first.created_at < second.created_at || (first.created_at === second.created_at && first.id < second.id));
  const missing = await reader.callTool({
    name: "kanlo_list_comments", arguments: { ticket_id: "missing-ticket" },
  });
  assert.equal(missing.isError, true);
  assert.match(missing.content[0].text, /404/);
  const move = { ticket_id: ticket.public_id, board_id, column_id: null, revision: update.revision };
  const staleMove = await writer.callTool({ name: "kanlo_move_ticket", arguments: move });
  assert.equal(staleMove.isError, true);
  assert.match(staleMove.content[0].text, /409/);
  const latest = await reader.callTool({
    name: "kanlo_get_ticket", arguments: { ticket_id: ticket.public_id },
  });
  move.revision = latest.structuredContent.ticket.revision;
  const moved = await writer.callTool({ name: "kanlo_move_ticket", arguments: move });
  assert.equal(moved.isError, undefined, JSON.stringify(moved));
  const backlog = await reader.callTool({
    name: "kanlo_list_tickets", arguments: { board_id, status: "backlog" },
  });
  assert.equal(backlog.structuredContent.items[0].id, ticket.id);
  assert.equal(backlog.structuredContent.items[0].column_id, null);
  const target = columns.structuredContent.items.find((column) => column.is_done);
  assert.ok(target);
  const completed = await writer.callTool({
    name: "kanlo_move_ticket",
    arguments: { ...move, revision: backlog.structuredContent.items[0].revision, column_id: target.id },
  });
  assert.equal(completed.isError, undefined, JSON.stringify(completed));
  const done = await reader.callTool({
    name: "kanlo_list_tickets", arguments: { board_id, column_id: target.id, status: "done" },
  });
  assert.equal(done.structuredContent.items[0].id, ticket.id);
  assert.equal(done.structuredContent.items[0].done, true);
  const next = await writer.callTool({
    name: "kanlo_create_ticket", arguments: { board_id, column_id, title: "MCP second ticket" },
  });
  const firstPage = await reader.callTool({
    name: "kanlo_list_tickets", arguments: { board_id, limit: 1 },
  });
  assert.equal(firstPage.structuredContent.next_offset, 1);
  const secondPage = await reader.callTool({
    name: "kanlo_list_tickets", arguments: { board_id, offset: 1, limit: 1 },
  });
  assert.deepEqual(new Set([...firstPage.structuredContent.items, ...secondPage.structuredContent.items].map(item => item.id)),
    new Set([ticket.id, next.structuredContent.id]));
  assert.equal(secondPage.structuredContent.next_offset, null);
});
