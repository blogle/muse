import { expect, test } from '@playwright/test';

test('generic fixture fields remain selectable across scalar, boolean, and category kinds', async ({ page }) => {
  await page.goto('/');
  await page.selectOption('#step-select', '3');

  const displayField = page.getByLabel('Display field');
  await expect(displayField.locator('option')).toHaveText(['signed_signal', 'mask', 'region']);

  await displayField.selectOption('signed_signal');
  await expect(page.locator('#range')).toContainText('Scalar');
  await displayField.selectOption('mask');
  await expect(page.locator('#range')).toContainText('Bool');
  await displayField.selectOption('region');
  await expect(page.locator('#range')).toContainText('Category');
  await expect(page.locator('#viewer canvas')).toBeVisible();
});

test('consolidated generic rendering applies metadata and channels without domain-specific field names', async ({ page }) => {
  test.skip(true, 'Depends on renderChannels.ts implementing generic metadata-driven channel selection and a consolidated default render entry point.');
  await page.goto('/');
  await expect(page.getByRole('group', { name: 'Render channels' })).toBeVisible();
  await expect(page.getByLabel('geometry_displacement')).toBeVisible();
  await expect(page.getByLabel('base_material')).toBeVisible();
  await expect(page.getByLabel('material_override')).toBeVisible();
  await expect(page.getByLabel('line_overlay')).toBeVisible();
  await expect(page.getByLabel('scalar_overlay')).toBeVisible();
  await expect(page.getByLabel('vector_overlay')).toBeVisible();
  await expect(page.locator('#viewer canvas')).toBeVisible();
});

test('causal overlay selection exposes generic scalar, vector, and network causes', async ({ page }) => {
  test.skip(true, 'Depends on causalOverlays.ts implementing causal scalar/vector/network selection and disclosing direction-only versus magnitude encoding.');
  await page.goto('/');
  const overlays = page.getByRole('group', { name: 'Causal overlays' });
  await expect(overlays).toBeVisible();
  await expect(overlays.getByRole('combobox', { name: 'Overlay' })).toBeVisible();
  await expect(overlays.getByText('Direction only')).toBeVisible();
  await expect(overlays.getByText('Magnitude')).toBeVisible();
});

test('raw inspection shows values and immediate provenance for every field kind', async ({ page }) => {
  test.skip(true, 'Depends on rawInspector.ts exposing per-cell scalar, vector, bool, category, index, and network values plus immediate upstream node IDs.');
  await page.goto('/');
  const inspector = page.getByRole('region', { name: 'Raw inspection' });
  await expect(inspector).toBeVisible();
  for (const kind of ['Scalar', 'Vector', 'Bool', 'Category', 'Index', 'Network']) {
    await expect(inspector.getByText(kind, { exact: true })).toBeVisible();
  }
  await expect(inspector.getByText(/upstream node/i)).toBeVisible();
});

test('field and network legends describe every generic field kind', async ({ page }) => {
  test.skip(true, 'Depends on metadata-driven legends for scalar, vector, bool, category, index, and network data.');
  await page.goto('/');
  const legend = page.getByRole('region', { name: 'Legend' });
  await expect(legend).toBeVisible();
  for (const kind of ['Scalar', 'Vector', 'Bool', 'Category', 'Index', 'Network']) {
    await expect(legend.getByText(kind, { exact: true })).toBeVisible();
  }
});

test('vector rendering discloses mode and provenance responds immediately to selection', async ({ page }) => {
  test.skip(true, 'Depends on vector overlay controls disclosing encoding mode and raw provenance updating immediately when a selection changes.');
  await page.goto('/');
  await page.getByRole('combobox', { name: 'Vector field' }).selectOption('flow');
  const mode = page.getByRole('group', { name: 'Vector rendering mode' });
  await expect(mode.getByText('Direction only')).toBeVisible();
  await expect(mode.getByText('Magnitude')).toBeVisible();
  const provenance = page.getByRole('region', { name: 'Provenance' });
  await page.getByRole('combobox', { name: 'Display field' }).selectOption('signed_signal');
  await expect(provenance).toContainText('signed_signal');
  await page.getByRole('combobox', { name: 'Display field' }).selectOption('region');
  await expect(provenance).toContainText('region');
});
