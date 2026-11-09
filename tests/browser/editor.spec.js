import { test, expect } from "@playwright/test";
import { randomUUID } from "node:crypto";
import {
  origin,
  fixture,
  visit,
  create,
  patch,
  ticket,
  mutation,
  closeDialog,
  resetLocale,
  queueSize,
} from "./helpers.js";

test.beforeEach(async ({ request }) => resetLocale(request));

test("ticket fields, type-independent custom fields, labels, sanitized Markdown and comments", async ({
  page,
  request,
}) => {
  const board = await fixture(request, "Editor");
  const type = {
    id: randomUUID(),
    name: `Bug ${board.prefix}`,
    color: "#aa3311",
    icon: "!",
    rank: 10,
    allow_children: true,
  };
  const label = {
    id: randomUUID(),
    name: `Global ${board.prefix}`,
    color: "#aa6611",
  };
  await mutation(request, { kind: "upsert_type", value: type });
  await mutation(request, { kind: "upsert_label", value: label });
  await mutation(
    request,
    {
      kind: "set_board_options",
      board_id: board.id,
      options: {
        ...board.snapshot.workspace.board_options[board.id],
        enabled_types: [type.id],
      },
    },
    board.snapshot.workspace.board_options[board.id],
  );
  const created = await create(request, board, "Detailed ticket");
  await visit(page, board);
  await page
    .getByRole("button", { name: "Detailed ticket", exact: true })
    .click();
  const dialog = page.getByRole("dialog", { name: /Ticketdetails/ });
  await dialog
    .getByRole("combobox", { name: "Tickettyp", exact: true })
    .selectOption(type.id);
  await dialog.getByLabel(label.name, { exact: true }).check();
  await dialog
    .getByRole("button", { name: "Textfeld hinzufügen", exact: true })
    .click();
  await dialog
    .getByRole("textbox", { name: "Feldname", exact: true })
    .fill("Kundenreferenz");
  await dialog
    .getByRole("textbox", { name: "Textwert", exact: true })
    .fill("ABC-42");
  await dialog
    .getByLabel("Beschreibung (Markdown)", { exact: true })
    .fill(
      '# Sicher\n\n**Markdown**\n\n<script>window.__kanlo_xss=1</script><img src=x onerror="window.__kanlo_xss=2">',
    );
  await dialog.getByRole("button", { name: "Vorschau", exact: true }).click();
  await expect(dialog.locator(".markdown script")).toHaveCount(0);
  await expect(dialog.locator(".markdown img")).toHaveCount(0);
  expect(await page.evaluate(() => window.__kanlo_xss)).toBeUndefined();
  await dialog.getByRole("button", { name: "Speichern", exact: true }).click();
  await expect
    .poll(
      async () =>
        (await ticket(request, created.id)).details.custom_fields
          .Kundenreferenz,
    )
    .toBe("ABC-42");
  await dialog
    .getByLabel("Neuer Kommentar (Markdown)", { exact: true })
    .fill("Ein **separater** Kommentar.");
  await dialog
    .getByRole("button", { name: "Kommentar speichern", exact: true })
    .click();
  await expect
    .poll(
      async () =>
        (
          await (
            await request.get(`${origin}/api/v1/boards/${board.id}/snapshot`)
          ).json()
        ).workspace.comments.length,
    )
    .toBe(1);
  await dialog
    .getByRole("combobox", { name: "Tickettyp", exact: true })
    .selectOption("");
  await dialog.getByRole("button", { name: "Speichern", exact: true }).click();
  await expect
    .poll(async () => (await ticket(request, created.id)).details.type_id)
    .toBeNull();
  expect(
    (await ticket(request, created.id)).details.custom_fields.Kundenreferenz,
  ).toBe("ABC-42");
  expect((await ticket(request, created.id)).details.labels).toContain(
    label.id,
  );
  await expect(dialog).toBeVisible();
  await closeDialog(dialog);
});

test("live updates never close the editor or overwrite unsaved fields", async ({
  page,
  request,
}) => {
  const board = await fixture(request, "Live editor");
  const created = await create(request, board, "Original title");
  await visit(page, board);
  await page
    .getByRole("button", { name: "Original title", exact: true })
    .click();
  const dialog = page.getByRole("dialog", { name: /Ticketdetails/ });
  await dialog
    .getByLabel("Titel", { exact: true })
    .fill("Mein ungespeicherter Titel");
  await patch(request, created.id, { importance: 201 });
  await expect
    .poll(async () =>
      page.evaluate(async () => {
        const db = await new Promise((resolve) => {
          const request = indexedDB.open("kanlo-client-v1");
          request.onsuccess = () => resolve(request.result);
        });
        return new Promise((resolve) => {
          const request = db.transaction("bases").objectStore("bases").getAll();
          request.onsuccess = () => {
            resolve(
              request.result
                .flatMap((snapshot) => snapshot.workspace.tickets)
                .some((ticket) => ticket.importance === 201),
            );
            db.close();
          };
        });
      }),
    )
    .toBe(true);
  await expect(dialog).toBeVisible();
  await expect(dialog.getByLabel("Titel", { exact: true })).toHaveValue(
    "Mein ungespeicherter Titel",
  );
  await dialog.getByRole("button", { name: "Speichern", exact: true }).click();
  await expect
    .poll(async () => (await ticket(request, created.id)).title)
    .toBe("Mein ungespeicherter Titel");
  expect((await ticket(request, created.id)).importance).toBe(201);
  await expect.poll(() => queueSize(page)).toBe(0);
  await closeDialog(dialog);
});
