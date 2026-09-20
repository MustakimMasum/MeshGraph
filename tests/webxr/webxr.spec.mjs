import { expect, test } from "@playwright/test";

const graph = {
  mode: "citations",
  nodes: [
    { id: "W1", title: "Old graph paper", year: 1990, topic: "Knowledge graphs", citationCount: 12 },
    { id: "W2", title: "Middle graph paper", year: 2010, topic: "Knowledge graphs", citationCount: 120 },
    { id: "W3", title: "Recent vision paper", year: 2025, topic: "Computer vision", citationCount: 4 },
  ],
  links: [
    { source: "W2", target: "W1", kind: "citations", weight: 1 },
    { source: "W3", target: "W2", kind: "citations", weight: 1 },
  ],
};

async function loadConstellation(page, xrSupported = false) {
  await page.addInitScript((supported) => {
    Object.defineProperty(navigator, "xr", {
      configurable: true,
      value: {
        addEventListener() {},
        isSessionSupported: async (mode) => mode === "immersive-vr" && supported,
      },
    });
  }, xrSupported);
  await page.route("**/api/v1/citations/graph**", async (route) => {
    const mode = new URL(route.request().url()).searchParams.get("mode") || "citations";
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ ...graph, mode }),
    });
  });
  await page.route("**/api/v1/citations/paper/W2", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        id: "W2",
        title: "Middle graph paper",
        year: 2010,
        topic: "Knowledge graphs",
        genre: "article",
        abstractText: "A paper about semantic research networks.",
        doi: "https://doi.org/10.1000/example",
        pdfUrl: "https://example.test/paper.pdf",
        citationCount: 120,
      }),
    });
  });
  await page.goto("/");
  await page.locator("a-scene").waitFor({ state: "attached" });
  await page.waitForFunction(() => {
    const graphElement = document.querySelector("#citation-graph");
    return graphElement?.components?.["af-force-graph"]?.nodeMesh;
  });
}

test("renders all papers and links in two batched GPU objects", async ({ page }) => {
  await loadConstellation(page);
  const state = await page.evaluate(() => {
    const component = document.querySelector("#citation-graph").components["af-force-graph"];
    return {
      nodeCount: component.nodeMesh.count,
      isInstancedMesh: component.nodeMesh.isInstancedMesh,
      linkType: component.linkLines.type,
      graphDomChildren: document.querySelector("#citation-graph").children.length,
    };
  });

  expect(state).toEqual({
    nodeCount: 3,
    isInstancedMesh: true,
    linkType: "LineSegments",
    graphDomChildren: 0,
  });
  await expect(page.locator(".graph-overview")).toContainText("3 papers");
  await expect(page.locator(".graph-overview")).toContainText("2 relationships");
});

test("maps older papers to negative Z and recent papers to positive Z", async ({ page }) => {
  await loadConstellation(page);
  const z = await page.evaluate(() => {
    const positions = document.querySelector("#citation-graph").components["af-force-graph"].positions;
    return [positions[2], positions[5], positions[8]];
  });

  expect(z[0]).toBeCloseTo(-12);
  expect(z[1]).toBeGreaterThan(z[0]);
  expect(z[2]).toBeCloseTo(12);
});

test("instanced ray selection crosses into Leptos as a macro event", async ({ page }) => {
  await loadConstellation(page);
  await page.evaluate(() => {
    document.querySelector("#citation-graph").components["af-force-graph"].selectInstance(1);
  });

  await expect(page.locator("#paper-panel h2")).toHaveText("Middle graph paper");
  await expect(page.locator("#paper-panel")).toContainText("120 citations");
  await expect(page.locator("#spatial-paper-card")).toHaveCount(0);
  await page.locator(".panel-close").click();
  await expect(page.locator("#paper-panel")).toHaveCount(0);
});

test("switches SPARQL topology modes without paper DOM nodes", async ({ page }) => {
  await loadConstellation(page);
  await page.getByRole("button", { name: "Co-citation" }).click();
  await page.waitForFunction(() =>
    document
      .querySelector("#citation-graph")
      .getAttribute("af-force-graph")
      .endpoint.includes("mode=co-citation"),
  );
  await expect(page.getByRole("button", { name: "Co-citation" })).toHaveClass(/active/);
  expect(await page.locator("#citation-graph").locator("a-sphere").count()).toBe(0);
});

test("index sweep filtering updates visibility and the Leptos HUD", async ({ page }) => {
  await loadConstellation(page);
  const visible = await page.evaluate(() => {
    const component = document.querySelector("#citation-graph").components["af-force-graph"];
    component.setYearCutoff(0.5);
    return [...component.visible];
  });
  expect(visible).toEqual([1, 0, 0]);
  await expect(page.locator(".graph-overview")).toContainText("1990 – 2008");
});

test("retains WebXR hand tracking and controller raycasting", async ({ page }) => {
  await loadConstellation(page, true);
  const configuration = await page.evaluate(() => {
    const scene = document.querySelector("a-scene");
    return {
      webxr: scene.getAttribute("webxr"),
      controllers: [...document.querySelectorAll("[laser-controls]")].map((controller) => ({
        hand: controller.getAttribute("laser-controls").hand,
        objects: controller.getAttribute("raycaster").objects,
      })),
    };
  });

  expect(configuration.webxr.optionalFeatures).toEqual(
    expect.arrayContaining(["bounded-floor", "hand-tracking"]),
  );
  expect(configuration.controllers).toEqual([
    { hand: "left", objects: ".citation-graph" },
    { hand: "right", objects: ".citation-graph" },
  ]);
});
