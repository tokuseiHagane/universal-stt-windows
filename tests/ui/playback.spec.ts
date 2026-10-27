import { expect, test } from "@playwright/test";

async function openAudio(page: import("@playwright/test").Page) {
  await page.goto("/");
  await page.getByTestId("demo-audio").click();
  await expect(page.getByTestId("project")).toBeVisible();
}

test("clicking a transcript row seeks the player", async ({ page }) => {
  await openAudio(page);
  await page.getByTestId("segment-row").nth(2).click();
  await expect
    .poll(() => page.evaluate(() => document.getElementById("player")?.currentTime ?? -1))
    .toBeGreaterThan(1.5);
});

test("clicking the timeline seeks to the clicked position", async ({ page }) => {
  await openAudio(page);
  const box = await page.getByTestId("timeline").boundingBox();
  expect(box).not.toBeNull();
  if (!box) return;
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await expect
    .poll(() => page.evaluate(() => document.getElementById("player")?.currentTime ?? -1))
    .toBeGreaterThan(1.1);
});

test("video demo splits transcript and preview", async ({ page }) => {
  await page.goto("/");
  await page.getByTestId("demo-video").click();
  await expect(page.getByTestId("project")).toBeVisible();
  await expect(page.locator('.preview video')).toBeVisible();
});
