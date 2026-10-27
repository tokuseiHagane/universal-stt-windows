import { expect, test } from "@playwright/test";

test("system theme resolves to the OS color scheme", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "light" });
  await page.goto("/");
  await page.getByTestId("open-settings").click();
  await page.locator(".settings-grid select").nth(1).selectOption("system");
  await page.getByTestId("settings-save").click();
  await expect(page.getByTestId("settings")).toHaveCount(0);
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await expect(page.locator("body")).toHaveCSS("background-color", "rgb(243, 245, 248)");
  await expect(page.locator("body")).toHaveCSS("color", "rgb(23, 26, 31)");
});
