import { test, expect } from "@playwright/test";
import {
  fixture,
  visit,
  create,
  patch,
  ticket,
  mutation,
  queueSize,
  resetLocale,
} from "./helpers.js";
test.beforeEach(async ({ request }) => resetLocale(request));

test("filter-based bulk changes require preview and preserve the saved filter and manual ranks", async ({
  page,
  request,
}) => {
  const board = await fixture(request, "Bulk");
  const a = await create(request, board, "Selected A"),
    b = await create(request, board, "Selected B");
  const other = await create(request, board, "Excluded");
  await visit(page, board);
  await page
    .getByRole("searchbox", { name: "Tickets durchsuchen", exact: true })
    .fill("Selected");
  await page
    .getByRole("combobox", { name: "Sortierung", exact: true })
    .selectOption("title");
  await expect.poll(() => queueSize(page)).toBe(0);
  await expect(
    page.getByRole("button", { name: "Selected A", exact: true }),
  ).toBeVisible();
  // View persistence is intentionally debounced; wait for the observable server state.
  await expect
    .poll(async () => {
      const snapshot = await (
        await request.get(
          `http://127.0.0.1:18080/api/v1/boards/${board.id}/snapshot`,
        )
      ).json();
      return Object.values(snapshot.view_states).some(
        (view) =>
          view.board_id === board.id &&
          view.query === "Selected" &&
          view.sort === "title",
      );
    })
    .toBe(true);
  await page
    .getByRole("button", { name: "Suchtreffer auswählen", exact: true })
    .click();
  await page.getByRole("button", { name: /Sammelaktion \(2\)/ }).click();
  const bulk = page.getByRole("dialog", { name: "Sammelaktion", exact: true });
  await bulk.getByLabel("Neue Priorität (0–255)", { exact: true }).fill("222");
  await bulk
    .getByRole("button", { name: "Vorschau anfordern", exact: true })
    .click();
  const preview = page.getByRole("dialog", {
    name: "Änderung bestätigen",
    exact: true,
  });
  await expect(preview).toContainText("2 betroffene Tickets");
  expect((await ticket(request, a.id)).importance).toBe(100);
  await preview
    .getByRole("button", { name: "Bestätigen und ausführen", exact: true })
    .click();
  await expect
    .poll(async () => (await ticket(request, a.id)).importance)
    .toBe(222);
  expect((await ticket(request, b.id)).importance).toBe(222);
  expect((await ticket(request, other.id)).importance).toBe(100);
  expect((await ticket(request, a.id)).details.rank).toBe(a.details.rank);
  await page.reload();
  await expect(
    page.getByRole("searchbox", { name: "Tickets durchsuchen", exact: true }),
  ).toHaveValue("Selected");
  await expect(
    page.getByRole("combobox", { name: "Sortierung", exact: true }),
  ).toHaveValue("title");
  await expect(
    page.getByRole("button", { name: "Excluded", exact: true }),
  ).toHaveCount(0);
});

test("dragging and keyboard movement respect strict WIP and show its error", async ({
  page,
  request,
}, info) => {
  const board = await fixture(request, "Input methods");
  const a = await create(request, board, "Drag ticket");
  const b = await create(request, board, "WIP ticket");
  const column = board.snapshot.workspace.columns.find(
    (column) => column.name === "In Arbeit",
  );
  await mutation(
    request,
    {
      kind: "update_column",
      id: column.id,
      name: column.name,
      is_done: false,
      wip_limit: 1,
      wip_strict: true,
      color: "#24533e",
    },
    column,
  );
  await visit(page, board);
  const card = page
    .getByRole("article")
    .filter({
      has: page.getByRole("button", { name: "Drag ticket", exact: true }),
    });
  const target = page.getByRole("region", { name: "In Arbeit", exact: true });
  if (info.project.name === "mobile") {
    await card.scrollIntoViewIfNeeded();
    const handle = card.getByRole("button", {
      name: `${a.public_id} ziehen`,
      exact: true,
    });
    const source = await handle.boundingBox();
    const cdp = await page.context().newCDPSession(page);
    await cdp.send("Input.dispatchTouchEvent", {
      type: "touchStart",
      touchPoints: [
        {
          x: source.x + source.width / 2,
          y: source.y + source.height / 2,
          id: 0,
        },
      ],
    });
    await target.scrollIntoViewIfNeeded();
    const destination = await target.boundingBox();
    await cdp.send("Input.dispatchTouchEvent", {
      type: "touchMove",
      touchPoints: [{ x: destination.x + 20, y: destination.y + 40, id: 0 }],
    });
    await cdp.send("Input.dispatchTouchEvent", {
      type: "touchEnd",
      touchPoints: [],
    });
    await cdp.detach();
  } else await card.dragTo(target);
  await expect
    .poll(async () => (await ticket(request, a.id)).column_id)
    .toBe(column.id);
  await card.getByRole("combobox").focus();
  await page.keyboard.press("Home");
  await page.keyboard.press("Enter");
  await expect
    .poll(async () => (await ticket(request, a.id)).column_id)
    .toBeNull();
  await card.getByRole("combobox").selectOption(column.id);
  await expect
    .poll(async () => (await ticket(request, a.id)).column_id)
    .toBe(column.id);
  const second = page
    .getByRole("article")
    .filter({
      has: page.getByRole("button", { name: "WIP ticket", exact: true }),
    });
  await second.getByRole("combobox").selectOption(column.id);
  await expect(
    page.getByRole("button", { name: "Konflikte prüfen", exact: true }),
  ).toBeVisible({ timeout: 10000 });
  expect((await ticket(request, b.id)).column_id).toBeNull();
  await page
    .getByRole("button", { name: "Konflikte prüfen", exact: true })
    .click();
  await expect(
    page.getByRole("dialog", { name: "Lokale Änderungen" }),
  ).toContainText(/WIP/);
});

test("theme and DE/EN persist, and Undo zero suppresses the action toast", async ({
  page,
  request,
}) => {
  const board = await fixture(request, "Preferences");
  await mutation(
    request,
    {
      kind: "set_board_options",
      board_id: board.id,
      options: {
        ...board.snapshot.workspace.board_options[board.id],
        undo_depth: 0,
      },
    },
    board.snapshot.workspace.board_options[board.id],
  );
  await visit(page, board);
  await page
    .getByRole("button", { name: "Einstellungen", exact: true })
    .click();
  const dialog = page.getByRole("dialog", {
    name: "Einstellungen",
    exact: true,
  });
  await dialog
    .getByRole("combobox", { name: "Sprache", exact: true })
    .selectOption("en");
  await dialog
    .getByRole("combobox", { name: "Design", exact: true })
    .selectOption("dark");
  await dialog
    .getByRole("button", {
      name: "Allgemeine Einstellungen speichern",
      exact: true,
    })
    .click();
  await expect.poll(() => queueSize(page)).toBe(0);
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page
    .getByRole("dialog", { name: "Settings", exact: true })
    .getByRole("button", { name: "Close", exact: true })
    .last()
    .click();
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  await page
    .getByLabel("New ticket in backlog", { exact: true })
    .fill("No undo toast");
  await page
    .getByRole("button", { name: "Create ticket", exact: true })
    .click();
  await expect.poll(() => queueSize(page)).toBe(0);
  await expect(
    page.getByRole("button", { name: "Undo", exact: true }),
  ).toHaveCount(0);
});
