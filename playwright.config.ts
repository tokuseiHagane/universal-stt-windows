import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/ui",
  timeout: 30_000,
  expect: { timeout: 10_000 },
  use: {
    baseURL: "http://127.0.0.1:4173",
    viewport: { width: 1280, height: 800 },
  },
  webServer: {
    command: "bash scripts/serve-web.sh",
    url: "http://127.0.0.1:4173",
    reuseExistingServer: true,
    timeout: 240_000,
  },
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
});
