import { expect } from "@playwright/test";
import { randomUUID } from "node:crypto";
export const origin = "http://127.0.0.1:18080";
export async function mutation(request, command, base_context = null) {
  const health = await (await request.get(`${origin}/api/v1/health`)).json();
  const response = await request.post(`${origin}/api/v1/commands`, {
    headers: { Origin: origin },
    data: {
      op_id: randomUUID(),
      client_id: "browser-fixture",
      server_instance_id: health.instance_id,
      base_epoch: health.epoch,
      command,
      base_context,
    },
  });
  expect(response.ok(), await response.text()).toBeTruthy();
  return response.json();
}
export async function fixture(request, name = "Browser fixture") {
  const prefix = `T${randomUUID()
    .replaceAll("-", "")
    .slice(0, 10)
    .split("")
    .map((character) =>
      String.fromCharCode(65 + Number.parseInt(character, 16)),
    )
    .join("")}`;
  const board = (
    await mutation(request, {
      kind: "create_board",
      name: `${name} ${prefix}`,
      prefix,
    })
  ).result.entity_id;
  for (const [name, is_done] of [
    ["Offen", false],
    ["In Arbeit", false],
    ["Erledigt", true],
  ])
    await mutation(request, {
      kind: "create_column",
      board_id: board,
      name,
      is_done,
      wip_limit: null,
      wip_strict: false,
    });
  const snapshot = await (
    await request.get(`${origin}/api/v1/boards/${board}/snapshot`)
  ).json();
  return {
    id: board,
    prefix,
    name: snapshot.workspace.boards[0].name,
    snapshot,
  };
}
export async function visit(page, board) {
  await page.goto("/");
  await page
    .getByRole("combobox", { name: "Board auswählen", exact: true })
    .selectOption(board.id);
  await expect(page.getByRole("heading", { level: 1 })).toHaveText(board.name);
  await page.evaluate(async () => {
    await navigator.serviceWorker.ready;
  });
  await expect
    .poll(() => page.evaluate(() => !!navigator.serviceWorker.controller))
    .toBe(true);
}
export async function create(request, board, title = "A ticket") {
  const response = await mutation(request, {
    kind: "create_ticket",
    board_id: board.id,
    title,
    description: "",
    importance: 100,
    column_id: null,
    parent_id: null,
    type_id: null,
  });
  return await (
    await request.get(`${origin}/api/v1/tickets/${response.result.entity_id}`)
  ).json();
}
export async function ticket(request, id) {
  return (await request.get(`${origin}/api/v1/tickets/${id}`)).json();
}
export async function tickets(request, board) {
  return (
    await request.get(`${origin}/api/v1/boards/${board.id}/tickets`)
  ).json();
}
export async function patch(request, id, fields) {
  const current = await ticket(request, id);
  return mutation(request, {
    kind: "patch_ticket",
    id: current.id,
    base_revision: current.revision,
    fields,
  });
}
export async function queueSize(page) {
  return page.evaluate(
    () =>
      new Promise((resolve, reject) => {
        const request = indexedDB.open("kanlo-client-v1");
        request.onerror = () => reject(request.error);
        request.onsuccess = () => {
          const database = request.result;
          const count = database
            .transaction("queue")
            .objectStore("queue")
            .count();
          count.onsuccess = () => {
            resolve(count.result);
            database.close();
          };
        };
      }),
  );
}
export async function closeDialog(dialog) {
  await dialog
    .getByRole("button", { name: "Schließen", exact: true })
    .last()
    .click();
}
export async function resetLocale(request) {
  const bootstrap = await (
    await request.get(`${origin}/api/v1/bootstrap`)
  ).json();
  if (
    bootstrap.settings.language !== "de" ||
    bootstrap.settings.theme !== "light"
  )
    await mutation(
      request,
      {
        kind: "set_settings",
        settings: { ...bootstrap.settings, language: "de", theme: "light" },
      },
      bootstrap.settings,
    );
}
