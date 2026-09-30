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
  await expect(page.locator(".brand-lockup")).toContainText("Research Citation Constellation");
});

test("maps older papers to negative Z and recent papers to positive Z", async ({ page }) => {
  await loadConstellation(page);
  const { z, depth } = await page.evaluate(() => {
    const component = document.querySelector("#citation-graph").components["af-force-graph"];
    const positions = component.positions;
    return { z: [positions[2], positions[5], positions[8]], depth: component.yearDepth };
  });

  expect(depth).toBeGreaterThanOrEqual(3.5);
  expect(depth).toBeLessThanOrEqual(7);
  expect(z[0]).toBeCloseTo(-depth);
  expect(z[1]).toBeGreaterThan(z[0]);
  expect(z[2]).toBeCloseTo(depth);
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
  await page.getByRole("button", { name: "Settings", exact: true }).click();
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

test("index sweep filtering updates visibility and emits the year range", async ({ page }) => {
  await loadConstellation(page);
  const result = await page.evaluate(() => {
    const component = document.querySelector("#citation-graph").components["af-force-graph"];
    let range;
    window.addEventListener("citation-year-filter", event => { range = event.detail; }, { once: true });
    component.selectInstance(1);
    component.setYearCutoff(0.5);
    return { visible: [...component.visible], range, selected: component.selectedIndex };
  });
  expect(result).toEqual({ visible: [1, 0, 0], range: "1990 – 2008", selected: -1 });
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

test("uses Hyperion frames as an alternate snapping pointer", async ({ page }) => {
  await loadConstellation(page);
  await page.evaluate(() => {
    class MockWebSocket {
      constructor(url) {
        this.url = url;
        this.listeners = new Map();
        window.__hyperionTestSocket = this;
        queueMicrotask(() => this.emit("open", {}));
      }

      addEventListener(name, callback) {
        const listeners = this.listeners.get(name) || [];
        listeners.push(callback);
        this.listeners.set(name, listeners);
      }

      close() {
        this.emit("close", {});
      }

      emit(name, event) {
        for (const callback of this.listeners.get(name) || []) callback(event);
      }

      sendFrame(hands) {
        this.emit("message", { data: JSON.stringify({ type: "frame", hands }) });
      }
    }
    window.WebSocket = MockWebSocket;
  });

  await page.getByRole("button", { name: "Use Leap Motion infrared hand input" }).click();
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(page.locator(".input-status")).toContainText("Hyperion bridge connected");

  await page.evaluate(() => {
    const graphElement = document.querySelector("#citation-graph");
    const graph = graphElement.components["af-force-graph"];
    const scene = document.querySelector("#citation-scene");
    const camera = scene.camera;
    const point = new THREE.Vector3(
      graph.positions[3],
      graph.positions[4],
      graph.positions[5],
    );
    graph.nodeMesh.updateWorldMatrix(true, false);
    camera.updateWorldMatrix(true, false);
    point.applyMatrix4(graph.nodeMesh.matrixWorld).project(camera);
    const cursor = [
      (point.x * 0.5 + 0.5) * window.innerWidth,
      (-point.y * 0.5 + 0.5) * window.innerHeight,
    ];
    const indexTip = [
      (cursor[0] / window.innerWidth - 0.5) * 320,
      (1 - cursor[1] / window.innerHeight) * 220 + 100,
      0,
    ];
    const hand = (pinchStrength) => ({
      id: 7,
      chirality: "right",
      flags: 0,
      confidence: 1,
      pinchDistance: pinchStrength ? 8 : 50,
      pinchStrength,
      grabStrength: 0,
      palm: {
        position: indexTip,
        stabilizedPosition: indexTip,
        velocity: [0, 0, 0],
        normal: [0, -1, 0],
        direction: [0, 1, 0],
      },
      digits: [
        { extended: true, tip: [-20, 200, 0] },
        { extended: true, tip: indexTip },
        { extended: false, tip: [0, 220, 0] },
        { extended: false, tip: [10, 215, 0] },
        { extended: false, tip: [20, 205, 0] },
      ],
    });
    window.__hyperionTestSocket.sendFrame([hand(0)]);
    window.__hyperionTestSocket.sendFrame([hand(0.9)]);
    window.__hyperionTestSocket.sendFrame([hand(0)]);
  });

  await expect(page.locator("#paper-panel h2")).toHaveText("Middle graph paper");
  await expect(page.locator("#gesture-pointer")).toBeVisible();
});
