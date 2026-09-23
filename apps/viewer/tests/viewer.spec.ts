import { expect, test } from '@playwright/test';

test('loads canonical snapshots and supports generic field overlays', async ({ page }) => {
  const errors: string[] = [];
  page.on('console', (message) => { if (message.type() === 'error') errors.push(message.text()); });
  page.on('pageerror', (error) => errors.push(error.message));
  page.on('requestfailed', (request) => errors.push(`${request.url()}: ${request.failure()?.errorText}`));
  await page.goto('/');
  await expect(page.locator('#step-select option')).toHaveCount(2);
  await expect(page.locator('#viewer canvas')).toBeVisible();
  await expect(page.locator('#scalar-select option')).toHaveText(['elevation', 'precipitation']);
  await page.selectOption('#scalar-select', 'elevation');
  await expect(page.locator('#scalar-select')).toHaveValue('elevation');
  await page.screenshot({ path: 'test-results/viewer-elevation.png' });
  await page.selectOption('#scalar-select', 'precipitation');
  await expect(page.locator('#scalar-select')).toHaveValue('precipitation');
  await page.selectOption('#vector-select', 'wind');
  await page.selectOption('#network-select', 'rivers');
  await expect(page.locator('#viewer canvas')).toBeVisible();
  await page.screenshot({ path: 'test-results/viewer-overlays.png' });
  await page.selectOption('#step-select', '1');
  await expect(page.locator('#step-select')).toHaveValue('1');
  expect(errors).toEqual([]);
});
