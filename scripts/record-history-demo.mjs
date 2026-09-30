import { chromium } from "@playwright/test";
import { mkdir } from "node:fs/promises";

await mkdir("demo/jpl-history", { recursive: true });
const browser = await chromium.launch({ channel: "chrome", headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, recordVideo: { dir: "demo/jpl-history", size: { width: 1440, height: 900 } } });
const page = await context.newPage();
try {
  await page.goto("http://127.0.0.1:3000/?collection=history");
  await page.locator("#history-node h2").waitFor();
  await page.waitForFunction(() => document.querySelector("#history-labels button"));
  await page.waitForTimeout(1500);
  await page.screenshot({ path: "demo/jpl-history/overview.png" });
  await page.getByRole("button", { name: "Guided introduction", exact: true }).click();
  await page.getByRole("button", { name: "Next tour stop", exact: true }).click();
  await page.waitForTimeout(1000);
  await page.getByRole("button", { name: "Next tour stop", exact: true }).click();
  await page.waitForTimeout(1000);
  await page.getByRole("textbox", { name: "Search history", exact: true }).fill("supernova");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await page.locator("#history-results button").first().click();
  await page.locator(".history-note").scrollIntoViewIfNeeded();
  await page.waitForTimeout(1800);
  await page.screenshot({ path: "demo/jpl-history/source-note.png" });
  await page.getByRole("button", { name: "Inspect relationship" }).first().click();
  await page.locator("#history-link").scrollIntoViewIfNeeded();
  await page.waitForTimeout(1200);
  const video = page.video();
  await context.close();
  await video.saveAs("demo/jpl-history/walkthrough.webm");
} finally {
  await browser.close();
}
