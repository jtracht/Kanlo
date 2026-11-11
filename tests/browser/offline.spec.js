import { test, expect } from "@playwright/test";
import {
  fixture,
  visit,
  create,
  patch,
  ticket,
  tickets,
  closeDialog,
  queueSize,
  resetLocale,
} from "./helpers.js";

test.beforeEach(async ({ request }) => resetLocale(request));

test("a delayed acknowledgment cannot overwrite a newer cached server revision", async ({
  page,
  context,
  request,
}) => {
  const board = await fixture(request, "Delayed acknowledgment");
  await visit(page, board);
  let committed = false,
    release,
    drop = false;
  const held = new Promise((resolve) => {
    release = resolve;
  });
  await context.route("**/api/v1/sync/create", async (route) => {
    const response = await route.fetch();
    committed = true;
    await held;
    await route.fulfill({ response });
  });
  await context.route("**/api/v1/bootstrap", async (route) => {
    if (drop) {
      await context.setOffline(true);
      await route.abort("internetdisconnected");
    } else await route.continue();
  });
  await page
    .getByLabel("Neues Ticket im Backlog", { exact: true })
    .fill("Initial title");
  await page
    .getByRole("button", { name: "Ticket anlegen", exact: true })
    .click();
  await expect.poll(() => committed).toBe(true);
  const [created] = await tickets(request, board);
  await patch(request, created.id, { title: "Newest title" });
  await page
    .getByRole("button", { name: "Aktualisieren", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Newest title", exact: true }),
  ).toHaveCount(1);
  await expect.poll(() => queueSize(page)).toBe(0);
  drop = true;
  release();
  await expect.poll(() => page.evaluate(() => navigator.onLine)).toBe(false);
  await page.reload();
  await expect(
    page.getByRole("heading", { name: "Newest title", exact: true }),
  ).toHaveCount(1);
  await expect(
    page.getByRole("heading", { name: "Initial title", exact: true }),
  ).toHaveCount(0);
});

test("lost creation acknowledgment is reconciled before rendering and never rolls back a newer server value", async ({
  page,
  context,
  request,
}) => {
  const board = await fixture(request, "Lost acknowledgment");
  // Lose the event channel too: otherwise SSE can legitimately reconcile the
  // committed receipt before the simulated HTTP acknowledgment is dropped.
  await context.route("**/api/v1/events", (route) => route.abort("connectionreset"));
  await visit(page, board);
  let committed = false;
  await context.route("**/api/v1/sync/create", async (route) => {
    if (committed) {
      await route.abort("connectionreset");
      return;
    }
    await route.fetch();
    committed = true;
    await context.setOffline(true);
    await route.abort("internetdisconnected");
  });
  await page
    .getByLabel("Neues Ticket im Backlog", { exact: true })
    .fill("Unacknowledged local creation");
  await page
    .getByRole("button", { name: "Ticket anlegen", exact: true })
    .click();
  await expect.poll(() => committed).toBe(true);
  await expect.poll(() => queueSize(page)).toBe(1);
  const [created] = await tickets(request, board);
  expect(created).toBeTruthy();
  await patch(request, created.id, { title: "Newer server value" });
  await context.setOffline(false);
  await page.reload();
  await expect.poll(() => queueSize(page), { timeout: 20000 }).toBe(0);
  await expect(
    page.getByRole("heading", { name: "Newer server value", exact: true }),
  ).toHaveCount(1);
  await expect(
    page.getByRole("heading", {
      name: "Unacknowledged local creation",
      exact: true,
    }),
  ).toHaveCount(0);
  expect((await tickets(request, board)).length).toBe(1);
  await context.setOffline(true);
  await page.reload();
  await expect(
    page.getByRole("heading", { name: "Newer server value", exact: true }),
  ).toHaveCount(1);
});

test("offline edit survives reload and merges an independent server field", async ({
  page,
  context,
  request,
}) => {
  const board = await fixture(request, "Offline merge");
  const created = await create(request, board, "Basis");
  await visit(page, board);
  await context.setOffline(true);
  await page.getByRole("button", { name: "Basis", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: /Ticketdetails/ });
  await dialog.getByLabel("Titel", { exact: true }).fill("Offline gespeichert");
  await dialog.getByRole("button", { name: "Speichern", exact: true }).click();
  await expect.poll(() => queueSize(page)).toBe(1);
  await closeDialog(dialog);
  await page.reload();
  await expect(
    page.getByRole("button", { name: "Offline gespeichert", exact: true }),
  ).toBeVisible();
  await patch(request, created.id, { importance: 233 });
  await context.setOffline(false);
  await expect.poll(() => queueSize(page), { timeout: 20000 }).toBe(0);
  const synced = await ticket(request, created.id);
  expect(synced.title).toBe("Offline gespeichert");
  expect(synced.importance).toBe(233);
});

test("same-field conflict is shown and explicit server choice preserves independent local fields", async ({
  page,
  context,
  request,
}) => {
  const board = await fixture(request, "Conflict");
  const created = await create(request, board, "Basis");
  await visit(page, board);
  await context.setOffline(true);
  await page.getByRole("button", { name: "Basis", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: /Ticketdetails/ });
  await dialog.getByLabel("Titel", { exact: true }).fill("Lokaler Titel");
  await dialog
    .getByLabel("Beschreibung (Markdown)", { exact: true })
    .fill("Unabhängige lokale Beschreibung");
  await dialog.getByRole("button", { name: "Speichern", exact: true }).click();
  await closeDialog(dialog);
  await patch(request, created.id, { title: "Server-Titel" });
  await context.setOffline(false);
  await expect(
    page.getByRole("button", { name: "Konflikte prüfen", exact: true }),
  ).toBeVisible({ timeout: 20000 });
  expect((await ticket(request, created.id)).title).toBe("Server-Titel");
  await page
    .getByRole("button", { name: "Konflikte prüfen", exact: true })
    .click();
  const queue = page.getByRole("dialog", {
    name: "Lokale Änderungen",
    exact: true,
  });
  await expect(
    queue.getByRole("cell", { name: "Lokaler Titel", exact: true }),
  ).toBeVisible();
  await expect(
    queue.getByRole("cell", { name: "Server-Titel", exact: true }),
  ).toBeVisible();
  await queue
    .getByRole("button", {
      name: "Serverwerte für Konflikte behalten",
      exact: true,
    })
    .click();
  await expect.poll(() => queueSize(page)).toBe(0);
  const final = await ticket(request, created.id);
  expect(final.title).toBe("Server-Titel");
  expect(final.description).toBe("Unabhängige lokale Beschreibung");
});

test("discarding an offline edit restores the confirmed cached value without a network request", async ({
  page,
  context,
  request,
}) => {
  const board = await fixture(request, "Discard");
  await create(request, board, "Bestätigte Basis");
  await visit(page, board);
  await context.setOffline(true);
  await page
    .getByRole("button", { name: "Bestätigte Basis", exact: true })
    .click();
  const dialog = page.getByRole("dialog", { name: /Ticketdetails/ });
  await dialog.getByLabel("Titel", { exact: true }).fill("Verwerfen");
  await dialog.getByRole("button", { name: "Speichern", exact: true }).click();
  await closeDialog(dialog);
  await expect.poll(() => queueSize(page)).toBe(1);
  await page.getByRole("button", { name: /1 lokal/ }).click();
  const queue = page.getByRole("dialog", { name: "Lokale Änderungen" });
  page.once("dialog", (dialog) => dialog.accept());
  await queue
    .getByRole("button", { name: "Diese Änderung verwerfen", exact: true })
    .click();
  await expect.poll(() => queueSize(page)).toBe(0);
  await closeDialog(queue);
  await expect(
    page.getByRole("button", { name: "Bestätigte Basis", exact: true }),
  ).toBeVisible();
  await page.reload();
  await expect(
    page.getByRole("button", { name: "Bestätigte Basis", exact: true }),
  ).toBeVisible();
});

test("an acknowledged creation stays cached when the next network request fails", async ({
  page,
  context,
  request,
}) => {
  const board = await fixture(request, "Lost connection after ack");
  await visit(page, board);
  let drop = false;
  // Context routing also observes fetches made by the controlling service worker.
  await context.route("**/api/v1/sync/create", async (route) => {
    const response = await route.fetch();
    drop = true;
    await route.fulfill({ response });
  });
  await context.route("**/api/v1/bootstrap", async (route) => {
    if (drop) {
      await context.setOffline(true);
      await route.abort("internetdisconnected");
    } else await route.continue();
  });
  await page
    .getByLabel("Neues Ticket im Backlog", { exact: true })
    .fill("Nach Bestätigung offline");
  await page
    .getByRole("button", { name: "Ticket anlegen", exact: true })
    .click();
  await expect.poll(async () => (await tickets(request, board)).length).toBe(1);
  await expect.poll(() => queueSize(page)).toBe(0);
  await expect.poll(() => page.evaluate(() => navigator.onLine)).toBe(false);
  await page.reload();
  const card = page.getByRole("article").filter({
    has: page.getByRole("button", {
      name: "Nach Bestätigung offline",
      exact: true,
    }),
  });
  await expect(card).toBeVisible();
  await expect(card.locator(".ticket-id")).toContainText(`${board.prefix}-1`);
});
