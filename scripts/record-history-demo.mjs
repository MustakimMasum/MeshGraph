import { chromium } from "@playwright/test";
import { mkdir } from "node:fs/promises";
import { resolve, join } from "node:path";

const output = resolve(process.argv[2] || "demo/jpl-history");
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ channel: "chrome", headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, recordVideo: { dir: output, size: { width: 1440, height: 900 } } });
const page = await context.newPage();
try {
  await page.goto("http://127.0.0.1:3000/?collection=history");
  await page.locator("#history-panel h2").waitFor();
  await page.waitForFunction(() => document.querySelector("#history-labels button"));
  await page.waitForTimeout(1500);
  await page.screenshot({ path: join(output, "overview.png") });
  await page.getByRole("button", { name: "Guided introduction", exact: true }).click();
  await page.getByRole("button", { name: "Next tour stop", exact: true }).click();
  await page.waitForTimeout(1000);
  await page.getByRole("button", { name: "Next tour stop", exact: true }).click();
  await page.waitForTimeout(1000);
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await page.getByRole("textbox", { name: "Search history", exact: true }).fill("supernova");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await page.locator("#history-results button").first().click();
  await page.locator(".history-note").scrollIntoViewIfNeeded();
  await page.waitForTimeout(1800);
  await page.screenshot({ path: join(output, "source-note.png") });
  await page.getByRole("button", { name: "Inspect relationship" }).first().click();
  await page.locator("#history-link").scrollIntoViewIfNeeded();
  await page.waitForTimeout(1200);
  await page.evaluate(() => document.getElementById("history-scene").emit("enter-vr"));
  await page.locator('#history-xr-card [data-action="Connection"]').evaluate(el => el.emit("click"));
  await page.waitForFunction(() => document.querySelector("#history-xr-card [text]")?.getAttribute("text").value.includes("Original values:"));
  await page.screenshot({ path: join(output, "xr-relationship-simulation.png") });
  await page.waitForTimeout(1200);
  await page.evaluate(() => document.getElementById("history-scene").emit("exit-vr"));
  const video = page.video();
  await context.close();
  await video.saveAs(join(output, "walkthrough.webm"));
} finally {
  await browser.close();
}
