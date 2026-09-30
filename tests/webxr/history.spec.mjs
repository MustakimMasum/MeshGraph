import { expect, test } from "@playwright/test";

const root = "6b8f6622-c037-54f0-9c09-b98a6c2428f0";
async function openHistory(page) {
  await page.goto("/?collection=history");
  await expect(page.locator("#history-node h2")).toHaveText("JPL Computer Graphics Laboratory");
  await page.waitForFunction(() => document.querySelector("#citation-graph")?.components?.["af-force-graph"]?.nodes.length > 0);
}

test("history API preserves the source graph and exposes safe attachments", async ({ request }) => {
  const response = await request.get("/api/v1/history/graph");
  expect(response.ok()).toBeTruthy();
  const data = await response.json();
  expect([data.nodes.length, data.links.length, data.attachments.length]).toEqual([262, 418, 58]);
  expect(data.links.every(edge => edge.raw.Direction === edge.direction)).toBeTruthy();
  expect(data.attachments.filter(a => a.owner === data.brainId)).toHaveLength(1);
  const note = data.attachments.find(a => a.text.includes("supernova"));
  const text = await request.get(`/api/v1/history/attachments/${note.id}`);
  expect(text.headers()["content-type"]).toContain("text/plain");
  expect(await text.text()).toContain("supernova");
  const image = data.attachments.find(a => a.asset);
  expect((await request.get(`/api/v1/history/attachments/${image.id}`)).ok()).toBeTruthy();
  const unavailable = data.attachments.find(a => a.status === "unavailable");
  expect((await request.get(`/api/v1/history/attachments/${unavailable.id}`)).status()).toBe(404);
  expect((await request.get("/api/v1/history/nodes/not-a-uuid")).status()).toBe(400);
});

test("search a note, inspect its relationship, and restore exploration with Back", async ({ page }) => {
  await openHistory(page);
  await page.getByRole("textbox", { name: "Search history", exact: true }).fill("supernova");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await page.locator("#history-results button").first().click();
  await expect(page.locator("#history-node h2")).toHaveText("a galaxy for Man and the Cosmos");
  await expect(page.locator(".history-note")).toContainText("supernova");
  await page.getByRole("button", { name: "Inspect relationship" }).first().click();
  await expect(page.locator("#history-link")).toContainText("Direction unresolved");
  await page.getByRole("button", { name: "Back", exact: true }).click();
  await expect(page.locator("#history-node h2")).toHaveText("JPL Computer Graphics Laboratory");
  await page.getByRole("button", { name: "Forward", exact: true }).click();
  await expect(page.locator("#history-node h2")).toHaveText("a galaxy for Man and the Cosmos");
});

test("guided route and local viewpoint survive reload including camera position", async ({ page }) => {
  await openHistory(page);
  await page.getByRole("button", { name: "Guided introduction" }).click();
  await page.getByRole("button", { name: "Next tour stop" }).click();
  await expect(page.locator("#history-node h2")).toHaveText("People");
  await page.locator("summary").filter({ hasText: "Saved viewpoints" }).click();
  await page.getByRole("textbox", { name: "Viewpoint name" }).fill("People research");
  await page.evaluate(() => document.getElementById("rig").object3D.position.set(2, 3, 8));
  await page.getByRole("button", { name: "Save view", exact: true }).click();
  await expect(page.getByRole("button", { name: "People research", exact: true })).toBeVisible();
  await page.reload();
  await expect(page.locator("#history-node h2")).toHaveText("JPL Computer Graphics Laboratory");
  await page.locator("summary").filter({ hasText: "Saved viewpoints" }).click();
  await page.getByRole("button", { name: "People research", exact: true }).click();
  await expect(page.locator("#history-node h2")).toHaveText("People");
  await expect.poll(() => page.evaluate(() => document.getElementById("rig").object3D.position.toArray())).toEqual([2, 3, 8]);
});

test("ordinary path excludes organizational shortcuts and XR exposes readable details", async ({ page, request }) => {
  const data = await (await request.get("/api/v1/history/graph")).json();
  const galaxy = data.nodes.find(n => n.title === "a galaxy for Man and the Cosmos");
  const path = await (await request.get(`/api/v1/history/path?from=${root}&to=${galaxy.id}`)).json();
  expect(path.found).toBeTruthy();
  expect(path.steps.every(s => data.links.find(l => l.id === s.linkId).meaning === 1)).toBeTruthy();
  await openHistory(page);
  await page.waitForSelector("#history-xr-card", { state: "attached" });
  await page.evaluate(() => document.getElementById("history-scene").emit("enter-vr"));
  expect(await page.locator("#history-xr-card").evaluate(el => el.getAttribute("visible"))).toBe(true);
  expect(await page.locator("#history-xr-card [text]").first().evaluate(el => el.getAttribute("text").value)).toContain("JPL Computer Graphics Laboratory");
  await page.evaluate(() => document.getElementById("history-scene").emit("exit-vr"));
  await expect(page.locator("#history-panel")).toBeVisible();
  await page.screenshot({ path: "test-results/jpl-history-desktop.png" });
});
