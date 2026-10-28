import { test, expect } from '@playwright/test';

test('speaker lane seeks and reveals a distant transcript row while paused', async ({ page }) => {
  await page.goto('/?long=1');
  await page.getByTestId('demo-audio').click();

  const timeline = page.getByTestId('timeline');
  const box = await timeline.boundingBox();
  if (!box) throw new Error('timeline has no box');
  await timeline.click({ position: { x: box.width * 45.5 / 60, y: box.height - 12 } });

  const row = page.locator('#segment-45');
  await expect(row).toBeVisible();
  await expect(row).toHaveClass(/active/);
  const time = await page.locator('#player').evaluate((element: HTMLAudioElement) => element.currentTime);
  expect(time).toBeCloseTo(2.25, 1);
});
