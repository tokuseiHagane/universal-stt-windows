import { expect, test } from "@playwright/test";

test("welcome shows a dropzone and opens the audio demo", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("dropzone")).toBeVisible();
  await expect(page.getByTestId("demo-audio")).toBeVisible();
  await page.getByTestId("demo-audio").click();
  await expect(page.getByTestId("project")).toBeVisible();
  await expect(page.getByTestId("speaker-chip")).toHaveCount(2);
  await expect(page.getByTestId("segment-row").first()).toBeVisible();
  await expect(page.getByTestId("timeline")).toBeVisible();
});
