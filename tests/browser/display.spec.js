import { test, expect } from "@playwright/test";
import { create, fixture, origin, resetLocale, visit } from "./helpers.js";
import { readFile } from "node:fs/promises";
import { randomUUID } from "node:crypto";

test.beforeEach(async ({ request }) => resetLocale(request));

test("TV commands are operated from the remote, independently of the web ticket", async ({ page, request }) => {
  const board = await fixture(request, "Display");
  const ticket = await create(request, board, "Anzeige aus dem Browser");
  await visit(page, board);
  const read = async () => (await request.get(`${origin}/api/v1/display`)).json();
  const before = await read();
  await page.getByRole("button", { name: ticket.title, exact: true }).click();
  const details = page.getByRole("dialog", { name: /Ticketdetails/ });
  await expect(details).toBeVisible();
  expect(await read()).toEqual(before);
  await expect(details.getByRole("button", { name: "Auf TV anzeigen", exact: true })).toHaveCount(0);
  await details.getByRole("button", { name: "Schließen", exact: true }).last().click();
  await page.getByRole("button", { name: "TV-Fernbedienung", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "TV-Fernbedienung", exact: true });
  await dialog.getByRole("combobox", { name: "Ticket", exact: true }).selectOption(ticket.id);
  await dialog.getByLabel("TV-Dauer in Minuten").fill("0");
  await dialog.getByRole("button", { name: "Auf TV anzeigen", exact: true }).click();
  await expect.poll(async () => (await read()).overlays.at(-1)).toEqual({
    kind: "ticket", ticket_id: ticket.id,
  });
  expect((await read()).ticket_minutes[ticket.id]).toBe(0);
  expect((await read()).expires_at).toBeNull();
  await dialog.getByRole("combobox", { name: "Ansicht", exact: true }).selectOption("overview");
  await dialog.getByRole("combobox", { name: "TV-Design", exact: true }).selectOption("colorful");
  await dialog.getByRole("button", { name: "Anzeige speichern" }).click();
  await expect.poll(async () => (await read()).preferences.view).toBe("overview");
  await expect.poll(async () => (await read()).preferences.design).toBe("colorful");
  await expect(page.getByRole("heading", { level: 1 })).toHaveText(board.name);
});

test("the Android entry URL opens the TV remote directly", async ({ page }) => {
  await page.goto(`${origin}/?tv-remote`);
  await expect(page.getByRole("dialog", { name: "TV-Fernbedienung", exact: true })).toBeVisible();
});

test("the calendar adds a sprint and loads it after reopening", async ({ page, request }) => {
  const board = await fixture(request, "Calendar");
  await visit(page, board);
  await page.getByRole("button", { name: "Kalender", exact: true }).click();
  const calendar = page.getByRole("dialog", { name: "Kalender", exact: true });
  const title = `Sprint ${board.prefix}`;
  await calendar.getByLabel("Titel", { exact: true }).fill(title);
  await calendar.getByLabel("Beginn").fill("2030-04-03");
  await calendar.getByLabel("Ende").fill("2030-04-07");
  await calendar.getByRole("button", { name: "Hinzufügen", exact: true }).click();
  await expect(calendar.getByText(title, { exact: true })).toBeVisible();
  const response = await request.get(`${origin}/api/v1/calendar`);
  expect(await response.json()).toContainEqual(expect.objectContaining({ title, kind: "sprint", start: "2030-04-03", end: "2030-04-07" }));
});

test("sprint tickets stay in the backlog and inherit the milestone label", async ({ page, request }) => {
  const board = await fixture(request, "Sprint-Backlog");
  const ticket = await create(request, board, "Geplantes Sprint-Ticket");
  const calendarPost = (path, data) => request.post(`${origin}/api/v1/calendar${path}`, {
    headers: { Origin: origin }, data,
  });
  const milestone = await (await calendarPost("", { kind: "milestone", title: "Ziel Sprint-Backlog", end: "2030-05-01" })).json();
  const sprint = await (await calendarPost("", { kind: "sprint", title: "Sprint A", start: "2030-04-01", end: "2030-04-10", milestone_id: milestone.id })).json();
  expect((await calendarPost("/tickets", { ticket_id: ticket.id, entry_id: sprint.id })).ok()).toBeTruthy();
  await visit(page, board);
  const card = page.getByRole("region", { name: "Backlog" }).locator("article").filter({ hasText: ticket.title });
  await expect(card.getByText("Sprint · Sprint A", { exact: true })).toBeVisible();
  await expect(card.getByText("Meilenstein · Ziel Sprint-Backlog", { exact: true })).toBeVisible();
  const health = await (await request.get(`${origin}/api/v1/health`)).json();
  const move = await request.post(`${origin}/api/v1/commands`, {
    headers: { Origin: origin },
    data: {
      op_id: randomUUID(), client_id: "sprint-test",
      server_instance_id: health.instance_id,
      base_epoch: health.epoch,
      command: { kind: "move_ticket", id: ticket.id, base_revision: ticket.revision,
        board_id: board.id, column_id: board.snapshot.workspace.columns[0].id, activate_types: false, convert_types: {} },
    },
  });
  expect(move.status(), await move.text()).toBe(409);
  await page.getByRole("button", { name: "Kalender", exact: true }).click();
  const hierarchy = page.getByRole("dialog", { name: "Kalender", exact: true }).locator("details").first();
  await expect(hierarchy.locator("li").getByText(`${ticket.public_id} · ${ticket.title}`, { exact: true })).toBeVisible();
});

test.describe("Gitea calendar response", () => {
  test.use({ serviceWorkers: "block" });

  test("a milestone can prefill a local sprint", async ({ page }) => {
    await page.route("**/api/v1/calendar/gitea", (route) => route.fulfill({
      json: { configured: true, milestones: [{ id: 1, title: "Gitea-Ziel", end: null, closed: false }] },
    }));
    await page.goto(origin);
    await page.getByRole("button", { name: "Kalender", exact: true }).click();
    const calendar = page.getByRole("dialog", { name: "Kalender", exact: true });
    await calendar.getByText("Gitea-Meilensteine ohne Termin", { exact: true }).click();
    await calendar.getByRole("button", { name: "Sprint daraus planen" }).click();
    await expect(calendar.getByLabel("Titel", { exact: true })).toHaveValue("Gitea-Ziel");
    await expect(calendar.getByLabel("Beginn")).toHaveValue("");
  });
});

test("integration setup exports a credential-free Actions schema whose reads work with a bearer key", async ({ page, request }) => {
  const board = await fixture(request, "GPT Actions");
  const ticket = await create(request, board, `Actions fixture ${board.prefix}`);
  const response = await request.post(`${origin}/api/v1/security/keys`, {
    headers: { Origin: origin }, data: { name: "Browser integration test" },
  });
  expect(response.ok()).toBeTruthy();
  const { token, id } = await response.json();
  try {
    await page.goto("/");
    await page.getByRole("button", { name: "Einstellungen", exact: true }).click();
    await page.getByText("Integration einrichten", { exact: true }).click();
    await page.getByLabel("Verbindungsart").selectOption("actions");
    await page.getByLabel("API-Schlüssel zum Testen").fill(token);
    await page.getByRole("button", { name: "Schlüssel an diesem Kanlo prüfen" }).click();
    await expect(page.getByText("Kanlo hat diesen API-Schlüssel akzeptiert.")).toBeVisible();
    await expect(page.getByLabel("API-Schlüssel zum Testen")).toHaveValue("");
    await page.getByLabel("Geschützte HTTPS-Adresse").fill("https://kanlo.example.org");
    const [download] = await Promise.all([
      page.waitForEvent("download"),
      page.getByRole("button", { name: "Leseschema herunterladen" }).click(),
    ]);
    const body = await readFile(await download.path(), "utf8");
    const schema = JSON.parse(body);
    expect(schema.servers[0].url).toBe("https://kanlo.example.org");
    expect(schema.components.securitySchemes.kanloKey.scheme).toBe("bearer");
    expect(schema.paths["/api/v1/settings/categories"].get.operationId).toBe("readKanloSettings");
    expect(body).not.toContain(token);
    const paths = new Map(Object.entries(schema.paths).map(([path, methods]) => {
      expect(Object.keys(methods)).toEqual(["get"]);
      expect(methods.get.security).toEqual([{ kanloKey: [] }]);
      return [methods.get.operationId, path];
    }));
    expect([...paths.keys()].sort()).toEqual([
      "getKanloTicket", "listKanloBoards", "readKanloSettings",
      "searchKanloTickets", "verifyKanloConnection",
    ]);
    const get = async (operation, { id, params } = {}) => {
      const path = paths.get(operation).replace("{id}", encodeURIComponent(id ?? ""));
      const result = await request.get(origin + path, {
        params, headers: { Authorization: `Bearer ${token}` },
      });
      expect(result.ok(), operation).toBeTruthy();
      return result.json();
    };
    await get("verifyKanloConnection");
    expect(await get("listKanloBoards")).toEqual(expect.arrayContaining([
      expect.objectContaining({ id: board.id }),
    ]));
    const results = await get("searchKanloTickets", { params: { query: ticket.title } });
    expect(results.items.map(({ id }) => id)).toEqual([ticket.id]);
    expect(await get("getKanloTicket", { id: ticket.public_id })).toMatchObject({
      id: ticket.id, title: ticket.title,
    });
    const settingPage = await get("readKanloSettings", { params: { category: "boards", board_id: board.id } });
    expect(settingPage.names).toHaveLength(5);
    expect(settingPage.next_offset).toBe(5);
    expect(settingPage.values).toBeUndefined();
  } finally {
    await request.post(`${origin}/api/v1/security/keys/${id}/revoke`, { headers: { Origin: origin }, data: {} });
  }
});

test("manual update settings show the installed server version on request", async ({ page, request }) => {
  const health = await (await request.get(`${origin}/api/v1/health`)).json();
  await page.goto("/");
  await page.getByRole("button", { name: "Einstellungen", exact: true }).click();
  await page.getByText("Manuelles Server-Update", { exact: true }).click();
  await page.getByRole("button", { name: "Installierten Stand anzeigen" }).click();
  await expect(page.getByText(`Installierte Version ${health.version}`, { exact: true })).toBeVisible();
});
