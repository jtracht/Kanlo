import { test, expect } from '@playwright/test';

test('create, move and persist a ticket without script execution', async ({ page }, testInfo) => {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto('/');
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Kanlo');
  const title = `Browser ${testInfo.project.name} ${Date.now()} <img src=x onerror=alert(1)>`;
  await page.getByLabel('Neues Ticket im Backlog').fill(title);
  await page.getByRole('button', { name: 'Ticket anlegen' }).click();
  const card = page.getByRole('article').filter({ has: page.getByRole('heading', { name: title, exact: true }) });
  await expect(card).toBeVisible();
  await expect(card.locator('img')).toHaveCount(0);
  const select = card.getByRole('combobox');
  await select.selectOption({ label: 'In Arbeit' });
  await expect(page.locator('section').filter({ has: page.getByRole('heading', { name: /^In Arbeit ·/ }) }).getByRole('heading', { name: title, exact: true })).toBeVisible();
  await page.reload();
  await expect(page.getByRole('heading', { name: title, exact: true })).toBeVisible();
  expect(errors).toEqual([]);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath('board.png'), fullPage: true });
});
