import { expect, test, type Page } from "@playwright/test";

async function openAudio(page: Page) {
  await page.goto("/");
  await page.getByTestId("demo-audio").click();
  await expect(page.getByTestId("project")).toBeVisible();
}

test("renaming a speaker is undoable", async ({ page }) => {
  await openAudio(page);
  const name = page.getByTestId("speaker-name-input").first();
  await name.fill("Alice");
  await name.press("Enter");
  await expect(name).toHaveValue("Alice");
  await page.keyboard.press("Control+z");
  await expect(page.getByTestId("speaker-name-input").first()).toHaveValue("Speaker 1");
});

test("editing a segment and saving updates the save state", async ({ page }) => {
  await openAudio(page);
  const state = page.getByTestId("save-state");
  const savedText = await state.textContent();
  const row = page.getByTestId("segment-row").first();
  await row.dblclick();
  const editor = page.locator('textarea[data-testid="segment-text"]');
  await expect(editor).toBeVisible();
  await editor.fill("Edited line");
  await editor.press("Enter");
  await expect(editor).toHaveCount(0);
  await expect(row).toContainText("Edited line");
  await expect(state).not.toHaveText(savedText ?? "");
  await page.getByTestId("save-button").click();
  await expect(state).toHaveText(savedText ?? "");
});
