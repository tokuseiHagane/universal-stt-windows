import { expect, test } from "@playwright/test";

async function startProcessing(page: import("@playwright/test").Page) {
  await page.goto("/?processing=1");
  await page.getByTestId("dropzone").evaluate((element) => {
    const transfer = new DataTransfer();
    transfer.items.add(new File(["demo"], "demo.mp3", { type: "audio/mpeg" }));
    element.dispatchEvent(new DragEvent("drop", { dataTransfer: transfer, bubbles: true, cancelable: true }));
  });
  await expect(page.getByTestId("processing")).toBeVisible();
}

test("processing shows distinct phase and overall ETA in English", async ({ page }) => {
  await startProcessing(page);
  await expect(page.getByTestId("phase-eta")).toContainText("Phase remaining");
  await expect(page.getByTestId("overall-eta")).toContainText("Total remaining");
  await expect(page.getByTestId("phase-eta")).not.toContainText("—");
  await expect(page.getByTestId("overall-eta")).not.toContainText("—");
});

test("processing ETA labels are localized in Russian", async ({ page }) => {
  await startProcessing(page);
  await page.getByTestId("open-settings").click();
  await page.locator(".settings-grid select").first().selectOption("ru");
  await page.getByTestId("settings-save").click();
  await expect(page.getByTestId("phase-eta")).toContainText("Осталось по этапу");
  await expect(page.getByTestId("overall-eta")).toContainText("Осталось всего");
});
