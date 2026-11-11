import { test, expect } from "@playwright/test";
import { queueSize } from "./helpers.js";
const protectedOrigin = "https://kanlo.test:18443";
const local = "http://127.0.0.1:18082";
const passcode = "kanlo browser fixture passcode";
async function login(page) {
  const dialog = page.getByRole("dialog", {
    name: "Externen Zugang anmelden",
    exact: true,
  });
  await expect(dialog).toBeVisible();
  await dialog
    .getByLabel("Gemeinsamer Zugangscode", { exact: true })
    .fill(passcode);
  await dialog.getByRole("button", { name: "Anmelden", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Kanlo");
}

test("HTTPS PWA works offline and expired external sessions retain the durable queue", async ({
  page,
  context,
  request,
}) => {
  await page.addInitScript(() => {
    const Original = window.EventSource;
    window.EventSource = class extends Original {
      constructor(url, options) {
        super(url, options);
        this.addEventListener("open", () => { window.kanloSseOpened = true; });
      }
    };
  });
  await page.goto(protectedOrigin);
  await login(page);
  expect(
    await page.evaluate(
      () => location.protocol === "https:" && isSecureContext,
    ),
  ).toBe(true);
  await page.evaluate(async () => {
    await navigator.serviceWorker.ready;
  });
  await expect
    .poll(() => page.evaluate(() => !!navigator.serviceWorker.controller))
    .toBe(true);
  const cookie = (await context.cookies()).find(
    (cookie) => cookie.name === "__Host-kanlo",
  );
  expect(cookie.secure && cookie.httpOnly && cookie.sameSite === "Strict").toBe(
    true,
  );
  await expect.poll(() => page.evaluate(() => window.kanloSseOpened)).toBe(true);
  const response = await request.post(
    `${local}/api/v1/security/sessions/revoke`,
    { headers: { Origin: local }, data: {} },
  );
  expect(response.ok()).toBe(true);
  const auth = page.getByRole("dialog", {
    name: "Externen Zugang anmelden",
    exact: true,
  });
  await expect(auth).toBeVisible({ timeout: 10000 });
  await auth
    .getByRole("button", { name: "Lokal weiterarbeiten", exact: true })
    .click();
  await context.setOffline(true);
  const title = `HTTPS offline ${Date.now()}`;
  await page.getByLabel("Neues Ticket im Backlog", { exact: true }).fill(title);
  await page
    .getByRole("button", { name: "Ticket anlegen", exact: true })
    .click();
  await expect.poll(() => queueSize(page)).toBe(1);
  await page.reload();
  await expect(
    page.getByRole("button", { name: title, exact: true }),
  ).toBeVisible();
  await expect.poll(() => queueSize(page)).toBe(1);
  await context.setOffline(false);
  // A fresh page cannot use a revoked cookie, even though its local cache is available.
  await expect(auth).toBeVisible({ timeout: 20000 });
  await expect.poll(() => queueSize(page)).toBe(1);
  await login(page);
  await expect.poll(() => queueSize(page), { timeout: 20000 }).toBe(0);
  const boards = await (await request.get(`${local}/api/v1/boards`)).json();
  const tickets = await (
    await request.get(`${local}/api/v1/boards/${boards[0].id}/tickets`)
  ).json();
  expect(tickets.filter((ticket) => ticket.title === title)).toHaveLength(1);
  const persisted = await page.evaluate(
    () =>
      new Promise((resolve, reject) => {
        const open = indexedDB.open("kanlo-client-v1");
        open.onerror = () => reject(open.error);
        open.onsuccess = async () => {
          const db = open.result;
          const stores = Array.from(db.objectStoreNames);
          const transaction = db.transaction(stores);
          const all = await Promise.all(
            stores.map(
              (name) =>
                new Promise((resolve) => {
                  const query = transaction.objectStore(name).getAll();
                  query.onsuccess = () => resolve(query.result);
                }),
            ),
          );
          db.close();
          resolve(JSON.stringify(all));
        };
      }),
  );
  expect(persisted).not.toContain(passcode);
  expect(persisted).not.toContain(cookie.value);
  expect(persisted).not.toContain("secret_hash");
});

test.describe("initial SSE authorization failure", () => {
  // Routing must see the original request, while the separate PWA test above
  // keeps service workers fully enabled and verifies the durable offline queue.
  test.use({ serviceWorkers: "block" });
  test("a session revoked before SSE opens requests sign-in even with an empty queue", async ({ page, request }) => {
    let releaseEvents;
    const allowed = new Promise((resolve) => { releaseEvents = resolve; });
    await page.route("**/api/v1/events", async (route) => {
      await allowed;
      await route.continue();
    });
    await page.goto(protectedOrigin);
    await login(page);
    const response = await request.post(`${local}/api/v1/security/sessions/revoke`, {
      headers: { Origin: local }, data: {},
    });
    expect(response.ok()).toBe(true);
    const rejected = page.waitForResponse((response) =>
      response.url().endsWith("/api/v1/events") && response.status() === 401,
    );
    releaseEvents();
    await rejected;
    await expect(page.getByRole("dialog", { name: "Externen Zugang anmelden", exact: true })).toBeVisible();
    await expect.poll(() => queueSize(page)).toBe(0);
  });
});
