import { test, expect } from "@playwright/test";
import { fixture, visit, tickets, queueSize, resetLocale } from "./helpers.js";
test.beforeEach(async ({ request }) => resetLocale(request));

test("two tabs synchronize one causal offline queue without duplicate tickets", async ({
  page,
  context,
  request,
}) => {
  const board = await fixture(request, "Two tabs");
  await visit(page, board);
  const other = await context.newPage();
  await visit(other, board);
  await context.setOffline(true);
  await page
    .getByLabel("Neues Ticket im Backlog", { exact: true })
    .fill("Offline aus Tab A");
  await page
    .getByRole("button", { name: "Ticket anlegen", exact: true })
    .click();
  await expect(
    other.getByRole("button", { name: "Offline aus Tab A", exact: true }),
  ).toBeVisible();
  await other
    .getByLabel("Neues Ticket im Backlog", { exact: true })
    .fill("Offline aus Tab B");
  await other
    .getByRole("button", { name: "Ticket anlegen", exact: true })
    .click();
  await expect.poll(() => queueSize(page)).toBe(2);
  await page.reload();
  await expect(
    page.getByRole("button", { name: "Offline aus Tab B", exact: true }),
  ).toBeVisible();
  await context.setOffline(false);
  await expect.poll(() => queueSize(page), { timeout: 20000 }).toBe(0);
  const final = await tickets(request, board);
  expect(final.map((ticket) => ticket.title).sort()).toEqual([
    "Offline aus Tab A",
    "Offline aus Tab B",
  ]);
  expect(new Set(final.map((ticket) => ticket.public_id)).size).toBe(2);
  await expect(
    other.locator(".ticket-id").filter({ hasText: `${board.prefix}-2` }),
  ).toBeVisible();
});

test("only visited board ticket data is cached", async ({ page, request }) => {
  const visited = await fixture(request, "Visited");
  await visit(page, visited);
  // Create this board after choosing the landing board: it cannot have been
  // automatically opened as the initial board in a fresh browser profile.
  const unvisited = await fixture(request, "Unvisited");
  await expect(
    page
      .getByRole("combobox", { name: "Board auswählen", exact: true })
      .locator(`option[value="${unvisited.id}"]`),
  ).toHaveCount(1);
  const keys = await page.evaluate(
    () =>
      new Promise((resolve) => {
        const open = indexedDB.open("kanlo-client-v1");
        open.onsuccess = () => {
          const db = open.result;
          const query = db
            .transaction("bases")
            .objectStore("bases")
            .getAllKeys();
          query.onsuccess = () => {
            resolve(query.result);
            db.close();
          };
        };
      }),
  );
  expect(keys).toContain(visited.id);
  expect(keys).not.toContain(unvisited.id);
});
