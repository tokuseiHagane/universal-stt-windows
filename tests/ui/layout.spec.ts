import { expect, test } from "@playwright/test";

test("project body gets the remaining height", async ({ page }, testInfo) => {
  await page.goto("/");
  await page.getByTestId("demo-video").click();
  await expect(page.getByTestId("project")).toBeVisible();

  const speakerHeight = await page.locator(".speaker-bar").evaluate((el) => el.getBoundingClientRect().height);
  const bodyHeight = await page.locator(".project-body").evaluate((el) => el.getBoundingClientRect().height);
  const transcriptHeight = await page.getByTestId("transcript").evaluate((el) => el.getBoundingClientRect().height);

  expect(speakerHeight).toBeLessThan(60);
  expect(bodyHeight).toBeGreaterThan(400);
  expect(transcriptHeight).toBeGreaterThan(400);
  await page.screenshot({ path: testInfo.outputPath("video-1280.png"), fullPage: true });
});

test("video keeps its aspect ratio inside the preview", async ({ page }) => {
  await page.goto("/");
  await page.getByTestId("demo-video").click();
  const video = page.locator(".preview video");
  await expect(video).toBeVisible();

  const fits = await video.evaluate((el) => {
    const videoRect = el.getBoundingClientRect();
    const previewRect = el.parentElement!.getBoundingClientRect();
    return videoRect.left >= previewRect.left - 1
      && videoRect.top >= previewRect.top - 1
      && videoRect.right <= previewRect.right + 1
      && videoRect.bottom <= previewRect.bottom + 1
      && getComputedStyle(el).objectFit === "contain";
  });
  expect(fits).toBe(true);
});

test("speaker names stay readable when no filter is selected", async ({ page }) => {
  await page.goto("/");
  await page.getByTestId("demo-audio").click();
  await expect(page.getByTestId("speaker-chip")).toHaveCount(2);
  await expect(page.locator(".speaker-chip.dim, .chip.dim")).toHaveCount(0);
  await expect(page.getByTestId("speaker-name-input").first()).toBeEditable();
});

test("1024px light theme has no horizontal overflow", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1024, height: 700 });
  await page.goto("/");
  await page.getByTestId("demo-audio").click();
  await page.getByTestId("open-settings").click();
  await page.getByTestId("settings").locator("select").nth(1).selectOption("light");
  await page.getByTestId("settings-save").click();
  await expect(page.getByTestId("settings")).toHaveCount(0);

  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth);
  expect(overflow).toBe(false);
  await expect(page.locator(":root")).toHaveAttribute("data-theme", "light");
  await page.screenshot({ path: testInfo.outputPath("audio-1024-light.png"), fullPage: true });
});
