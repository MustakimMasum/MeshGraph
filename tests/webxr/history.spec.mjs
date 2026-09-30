import { expect, test } from "@playwright/test";

const root = "6b8f6622-c037-54f0-9c09-b98a6c2428f0";
async function openHistory(page) {
  await page.goto("/?collection=history");
  await expect(page.locator("#history-panel h2")).toHaveText("JPL Computer Graphics Laboratory");
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
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await page.getByRole("textbox", { name: "Search history", exact: true }).fill("supernova");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await page.locator("#history-results button").first().click();
  await expect(page.locator("#history-panel h2")).toHaveText("a galaxy for Man and the Cosmos");
  await expect(page.locator(".history-note")).toContainText("supernova");
  await page.getByRole("button", { name: "Inspect relationship" }).first().click();
  await expect(page.locator("#history-link")).toContainText("Direction unresolved");
  await page.getByRole("button", { name: "Back", exact: true }).click();
  await expect(page.locator("#history-panel h2")).toHaveText("JPL Computer Graphics Laboratory");
  await page.getByRole("button", { name: "Forward", exact: true }).click();
  await expect(page.locator("#history-panel h2")).toHaveText("a galaxy for Man and the Cosmos");
});

test("guided route and local viewpoint survive reload including camera position", async ({ page }) => {
  await openHistory(page);
  await page.getByRole("button", { name: "Guided introduction" }).click();
  await page.getByRole("button", { name: "Next tour stop" }).click();
  await expect(page.locator("#history-panel h2")).toHaveText("People");
  await page.locator("summary").filter({ hasText: "Saved viewpoints" }).click();
  await page.getByRole("textbox", { name: "Viewpoint name" }).fill("People research");
  await page.evaluate(() => document.getElementById("rig").object3D.position.set(2, 3, 8));
  await page.getByRole("button", { name: "Save view", exact: true }).click();
  await expect(page.getByRole("button", { name: "People research", exact: true })).toBeVisible();
  await page.reload();
  await expect(page.locator("#history-panel h2")).toHaveText("JPL Computer Graphics Laboratory");
  await page.locator("summary").filter({ hasText: "Saved viewpoints" }).click();
  await page.getByRole("button", { name: "People research", exact: true }).click();
  await expect(page.locator("#history-panel h2")).toHaveText("People");
  await expect.poll(() => page.evaluate(() => document.getElementById("rig").object3D.position.toArray())).toEqual([2, 3, 8]);
});

test("ordinary path excludes organizational shortcuts and XR exposes readable details", async ({ page, request }, testInfo) => {
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
  await page.screenshot({ path: testInfo.outputPath("jpl-history-desktop.png") });
});

test("XR connection controls expose source values and preserve the graph transform", async ({ page, request }) => {
  const data = await (await request.get("/api/v1/history/graph")).json();
  const links = data.links.filter(edge => edge.source === root || edge.target === root);
  await openHistory(page);
  await page.waitForSelector('#history-xr-card [data-action="Connection"]', { state: "attached" });
  await page.evaluate(() => {
    document.getElementById("citation-graph").object3D.position.set(1, 2, 3);
    document.getElementById("history-scene").emit("enter-vr");
  });
  const text = () => page.locator("#history-xr-card [text]").first().evaluate(el => el.getAttribute("text").value);
  const press = action => page.locator(`#history-xr-card [data-action="${action}"]`).evaluate(el => el.emit("click"));
  for (const edge of [...links, links[0]]) {
    await press("Connection");
    await expect.poll(text).toContain(`Relation ${edge.relation}; Meaning ${edge.meaning}; Direction ${edge.direction}`);
    await expect.poll(text).toContain(edge.id);
    await expect.poll(text).toContain(edge.directionLabel);
  }
  expect(await page.locator("#citation-graph").evaluate(el => el.object3D.position.toArray())).toEqual([1, 2, 3]);
  await press("Topic");
  await expect.poll(text).toContain("JPL Computer Graphics Laboratory");
  await expect.poll(text).not.toContain("Original values:");
  await press("Next page");
  await expect.poll(text).toContain("Page 2/");
  await press("Home");
  await expect(page.locator("#history-panel h2")).toHaveText("JPL Computer Graphics Laboratory");
  await page.evaluate(() => document.getElementById("history-scene").emit("exit-vr"));
});

test("history gesture simulation pans, zooms, and selects through the shared input bridge", async ({ page }) => {
  await openHistory(page);
  const result = await page.evaluate(() => {
    const graphElement = document.getElementById("citation-graph");
    const graph = graphElement.components["af-force-graph"];
    const input = document.getElementById("history-scene").components["gesture-controls"];
    const position = graphElement.object3D.position.toArray();
    const scale = graphElement.object3D.scale.x;
    input.applyGesture({ kind: "palm_pan", dx: 0.1, dy: 0.1 });
    input.applyGesture({ kind: "spread", delta: 0.05 });
    const target = graph.nodes.findIndex(node => node.id !== graph.lastSelectedId);
    const title = graph.nodes[target].title;
    const after = graphElement.object3D.position.toArray();
    const zoom = graphElement.object3D.scale.x;
    input.hoveredInstanceId = target;
    input.applyGesture({ kind: "pinch", x: 0.5, y: 0.5 });
    return { position, after, scale, zoom, title };
  });
  expect(result.after[0]).toBeCloseTo(result.position[0] + 1.2);
  expect(result.after[1]).toBeCloseTo(result.position[1] - 1);
  expect(result.zoom).toBeGreaterThan(result.scale);
  await expect(page.locator("#history-panel h2")).toHaveText(result.title);
});


test("JPL IR focused pinch selects without panning and empty-space pinch pans", async ({ page }) => {
  await openHistory(page);
  await page.waitForFunction(() => document.querySelector("#citation-graph")?.components?.["af-force-graph"]?.nodeMesh);
  const focused = await page.evaluate(() => {
    const scene = document.getElementById("history-scene");
    const graph = document.getElementById("citation-graph").components["af-force-graph"];
    const input = scene.components["gesture-controls"];
    input.inputSource = "hyperion";
    input.resetHyperionNavigation();
    graph.nodeMesh.updateWorldMatrix(true, false);
    scene.camera.updateWorldMatrix(true, false);
    const target = graph.nodes.findIndex((node, index) => {
      if (node.id === graph.lastSelectedId || !input.projectNodeToScreen(graph, index, scene.camera, innerWidth, innerHeight)) return false;
      return input.projectedNode.x > 390 && input.projectedNode.x < innerWidth - 30 &&
        input.projectedNode.y > 30 && input.projectedNode.y < innerHeight - 90;
    });
    if (target < 0) throw new Error("No visible JPL selection target");
    input.projectNodeToScreen(graph, target, scene.camera, innerWidth, innerHeight);
    const position = [(input.projectedNode.x / innerWidth - 0.5) * 320,
      (1 - input.projectedNode.y / innerHeight) * 220 + 100, 0];
    const hand = (pinchStrength, dx = 0) => ({
      id: 71, chirality: "right", confidence: 0.2, pinchStrength, grabStrength: 0.8,
      palm: { position: [position[0] + dx, position[1], 0] },
      digits: Array.from({ length: 5 }, () => ({ extended: false })),
    });
    window.__jplIRHand = hand;
    input.applyHyperionFrame([hand(0)]);
    const hover = input.hoveredInstanceId;
    if (hover !== target) throw new Error(`Focus missed: ${hover} instead of ${target}`);
    input.applyHyperionFrame([hand(0.9)]);
    return { title: graph.nodes[target].title, selected: graph.lastSelectedId, expected: graph.nodes[target].id };
  });
  expect(focused.selected).toBe(focused.expected);
  await expect(page.locator("#history-panel h2")).toHaveText(focused.title);
  const drag = await page.evaluate(() => {
    const input = document.getElementById("history-scene").components["gesture-controls"];
    const object = document.getElementById("citation-graph").object3D;
    const before = object.position.clone();
    input.applyHyperionFrame([window.__jplIRHand(0.9, 0.5)]);
    const delta = object.position.clone().sub(before).length();
    input.applyHyperionFrame([window.__jplIRHand(0.9, 0.5)]);
    const stationaryDelta = object.position.clone().sub(before).length();
    input.applyHyperionFrame([window.__jplIRHand(0, 0.5)]);
    const emptyHand = (pinchStrength, dx = 0) => ({ id: 71, chirality: "right",
      pinchStrength, grabStrength: 0, palm: { position: [155 + dx, 105, 0] },
      digits: Array.from({ length: 5 }, () => ({ extended: false })) });
    input.applyHyperionFrame([emptyHand(0)]);
    if (Number.isInteger(input.hoveredInstanceId)) throw new Error("Expected empty space for pan");
    const panBefore = object.position.clone();
    input.applyHyperionFrame([emptyHand(0.9)]);
    input.applyHyperionFrame([emptyHand(0.9, 0.5)]);
    const emptyPanDelta = object.position.clone().sub(panBefore).length();
    input.applyHyperionFrame([emptyHand(0)]);
    return { delta, stationaryDelta, emptyPanDelta };
  });
  expect(drag.delta).toBe(0);
  expect(drag.stationaryDelta).toBe(0);
  expect(drag.emptyPanDelta).toBeGreaterThan(0);
});
