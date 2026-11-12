import { test, expect } from "@playwright/test";
import { create, fixture, origin, resetLocale, visit } from "./helpers.js";
import { readFile } from "node:fs/promises";

test.beforeEach(async ({ request }) => resetLocale(request));

test("opening a web ticket preserves the independent TV view until explicitly sent", async ({ page, request }) => {
  const board = await fixture(request, "Display");
  const ticket = await create(request, board, "Anzeige aus dem Browser");
  await visit(page, board);
  const read = async () => (await request.get(`${origin}/api/v1/display`)).json();
  const before = await read();
  await page.getByRole("button", { name: ticket.title, exact: true }).click();
  const details = page.getByRole("dialog", { name: /Ticketdetails/ });
  await expect(details).toBeVisible();
  expect(await read()).toEqual(before);
  await details.getByRole("button", { name: "Auf TV anzeigen", exact: true }).click();
  await expect.poll(async () => (await read()).overlays.at(-1)).toEqual({
    kind: "ticket", ticket_id: ticket.id,
  });
  await expect(details).toBeVisible();
  await details.getByRole("button", { name: "Schließen", exact: true }).last().click();
  await page.getByRole("button", { name: "TV-Anzeige", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "TV-Anzeige", exact: true });
  await dialog.getByRole("combobox", { name: "Ansicht", exact: true }).selectOption("overview");
  await dialog.getByRole("button", { name: "Anzeige speichern" }).click();
  await expect.poll(async () => (await read()).preferences.view).toBe("overview");
  await expect(page.getByRole("heading", { level: 1 })).toHaveText(board.name);
});

test("integration setup verifies a bearer key and exports a credential-free Actions schema", async ({ page, request }) => {
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
    expect(body).not.toContain(token);
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
