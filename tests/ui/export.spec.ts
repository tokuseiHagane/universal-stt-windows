import { expect, test, type Page } from "@playwright/test";
import { readFile } from "node:fs/promises";

async function openExport(page: Page) {
  await page.goto("/");
  await page.getByTestId("demo-audio").click();
  await expect(page.getByTestId("project")).toBeVisible();
  await page.getByTestId("open-export").click();
  await expect(page.getByTestId("export-modal")).toBeVisible();
}

test("exports filtered VTT and downloads it", async ({ page }) => {
  await openExport(page);
  await page.getByTestId("export-format").selectOption("Vtt");
  const speakers = page.getByTestId("export-speaker");
  await expect(speakers).toHaveCount(2);
  await speakers.nth(1).uncheck();
  const downloadPromise = page.waitForEvent("download");
  await page.getByTestId("export-submit").click();
  const download = await downloadPromise;
  expect(download.suggestedFilename()).toMatch(/\.vtt$/);
  const path = await download.path();
  expect(path).toBeTruthy();
  const body = await readFile(path!, "utf8");
  expect(body.startsWith("WEBVTT")).toBe(true);
  expect(body).toContain("Speaker 1:");
  expect(body).not.toContain("Speaker 2:");
});
