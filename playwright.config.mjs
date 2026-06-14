import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/webxr",
  timeout: 30000,
  use: {
    baseURL: "http://127.0.0.1:3000",
    browserName: "chromium",
    channel: "chrome",
    headless: true,
  },
});

