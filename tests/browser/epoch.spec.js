import { test, expect } from "@playwright/test";
import { randomUUID } from "node:crypto";
import {
  origin,
  fixture,
  visit,
  create,
  ticket,
  queueSize,
  closeDialog,
  resetLocale,
} from "./helpers.js";
test.beforeEach(async ({ request }) => resetLocale(request));
test("a restore epoch retains old edits for explicit review and never resurrects a removed ticket", async ({
  page,
  context,
  request,
}) => {
  const board = await fixture(request, "Epoch");
  await create(request, board, "Keep");
  const backup = await (
    await request.post(`${origin}/api/v1/backups`, {
      headers: { Origin: origin },
      data: {},
    })
  ).json();
  const removed = await create(request, board, "After backup");
  await visit(page, board);
  await context.setOffline(true);
  await page.getByRole("button", { name: "After backup", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: /Ticketdetails/ });
  await dialog.getByLabel("Titel", { exact: true }).fill("Old queued edit");
  await dialog.getByRole("button", { name: "Speichern", exact: true }).click();
  await closeDialog(dialog);
  await expect.poll(() => queueSize(page)).toBe(1);
  const preview = await (
    await request.post(`${origin}/api/v1/backups/restore/preview`, {
      headers: { Origin: origin },
      data: { backup, boards: [board.id] },
    })
  ).json();
  expect(preview.token).toBeTruthy();
  const restored = await request.post(`${origin}/api/v1/recovery/confirm`, {
    headers: { Origin: origin },
    data: {
      token: preview.token,
      op_id: randomUUID(),
      client_id: "epoch-fixture",
    },
  });
  expect(restored.ok(), await restored.text()).toBe(true);
  await context.setOffline(false);
  await expect(
    page.getByRole("button", { name: "Konflikte prüfen", exact: true }),
  ).toBeVisible({ timeout: 20000 });
  await expect.poll(() => queueSize(page)).toBe(1);
  expect(
    (await request.get(`${origin}/api/v1/tickets/${removed.id}`)).status(),
  ).toBe(404);
  await page
    .getByRole("button", { name: "Konflikte prüfen", exact: true })
    .click();
  await expect(
    page.getByRole("dialog", { name: "Lokale Änderungen" }),
  ).toContainText("gelöschte Tickets werden nicht automatisch neu angelegt");
  await page.reload();
  await expect.poll(() => queueSize(page)).toBe(1);
  expect(
    (await request.get(`${origin}/api/v1/tickets/${removed.id}`)).status(),
  ).toBe(404);
});
