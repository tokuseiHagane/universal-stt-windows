import { expect, test } from "@playwright/test";

test("manual URL input imports a remote project", async ({ page }) => {
  await page.goto("/");
  await page.getByTestId("url-input").fill("  https://example.test/clip.mp4  ");
  await page.getByTestId("url-submit").click();
  await expect(page.getByTestId("processing")).toBeVisible();
  await expect(page.getByTestId("project")).toBeVisible();
  await expect(page.getByTestId("segment-row").first()).toBeVisible();
});

test("text URL drop imports a remote project", async ({ page }) => {
  await page.goto("/");
  const transfer = await page.evaluateHandle(() => {
    const value = new DataTransfer();
    value.setData("text/plain", "https://example.test/dropped.mp4");
    return value;
  });
  await page.getByTestId("dropzone").dispatchEvent("drop", { dataTransfer: transfer });
  await expect(page.getByTestId("processing")).toBeVisible();
  await expect(page.getByTestId("project")).toBeVisible();
});

test("invalid URL shows a localized error", async ({ page }) => {
  await page.goto("/");
  await page.getByTestId("url-input").fill("ftp://example.test/clip.mp4");
  await page.getByTestId("url-submit").click();
  await expect(page.getByTestId("url-error")).toContainText("HTTP");
  await expect(page.getByTestId("processing")).toHaveCount(0);

  await page.getByTestId("open-settings").click();
  await page.locator(".settings-grid select").first().selectOption("ru");
  await page.getByTestId("settings-save").click();
  await page.getByTestId("url-input").fill("javascript:alert(1)");
  await page.getByTestId("url-submit").click();
  await expect(page.getByTestId("url-error")).toContainText("HTTP");
});
