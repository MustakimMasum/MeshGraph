import { expect, test } from "@playwright/test";

test("deployed collections and webcam runtime work with all external browser requests blocked", async ({ page, context, baseURL }) => {
  test.setTimeout(90000);
  const origin = new URL(baseURL).origin;
  const externalRequests = [];
  await context.route("**/*", async (route) => {
    const url = new URL(route.request().url());
    if (["http:", "https:"].includes(url.protocol) && url.origin !== origin) {
      externalRequests.push(url.href);
      await route.abort();
    } else {
      await route.continue();
    }
  });

  await page.goto("/");
  await page.waitForFunction(() => document.querySelector("#citation-graph")?.components?.["af-force-graph"]?.nodeMesh?.count > 0);
  const graph = await page.request.get(`${origin}/api/v1/citations/graph`);
  expect(graph.ok()).toBeTruthy();
  const citations = await graph.json();
  expect(citations.nodes.length).toBeGreaterThan(0);
  expect(citations.links.length).toBeGreaterThan(0);

  const workerResult = await page.evaluate(() => new Promise((resolve, reject) => {
    const worker = new Worker("/public/hand-worker.js");
    const timeout = setTimeout(() => finish(new Error("Bundled hand model initialization timed out")), 45000);
    function finish(error, result) {
      clearTimeout(timeout);
      worker.terminate();
      if (error) reject(error);
      else resolve(result);
    }
    worker.onerror = (event) => finish(new Error(event.message));
    worker.onmessage = async ({ data }) => {
      if (data.type === "error") finish(new Error(data.message));
      if (data.type === "ready") {
        const canvas = new OffscreenCanvas(128, 128);
        const frame = canvas.transferToImageBitmap();
        worker.postMessage({ type: "frame", frame, timestamp: 1 }, [frame]);
      }
      if (data.type === "result") finish(null, { handCount: data.handCount });
    };
    worker.postMessage({ type: "init" });
  }));
  expect(workerResult.handCount).toBe(0);

  await page.goto("/?collection=history");
  await expect(page.locator("#history-panel h2")).toHaveText("JPL Computer Graphics Laboratory");
  await page.waitForFunction(() => document.querySelector("#citation-graph")?.components?.["af-force-graph"]?.nodeMesh?.count > 0);
  const summary = await page.request.get(`${origin}/api/v1/history/summary`);
  expect(summary.ok()).toBeTruthy();
  expect((await summary.json()).counts.thoughts).toBeGreaterThan(0);
  expect(externalRequests).toEqual([]);
});

test("Leap Motion opens its companion and reconnects without a command-line instruction", async ({ page }) => {
  await page.goto("/");
  await page.waitForFunction(() => document.querySelector("a-scene")?.components?.["gesture-controls"]);
  await page.evaluate(() => {
    const input = document.querySelector("a-scene").components["gesture-controls"];
    window.leapLaunches = 0;
    window.leapConnections = 0;
    input.launchHyperionCompanion = () => { window.leapLaunches++; input.showHyperionSetup(); };
    const NativeWebSocket = window.WebSocket;
    window.WebSocket = class {
      constructor(url) {
        if (!String(url).includes("127.0.0.1:6437")) return new NativeWebSocket(url);
        this.listeners = {};
        const attempt = ++window.leapConnections;
        setTimeout(() => {
          if (attempt === 1) { this.emit("error"); this.emit("close"); }
          else this.emit("open");
        }, 20);
      }
      addEventListener(name, callback) { this.listeners[name] = callback; }
      emit(name) { this.listeners[name]?.({}); }
      close() { this.emit("close"); }
    };
  });
  await page.getByRole("button", { name: "Use Leap Motion infrared hand input" }).click();
  await expect(page.getByRole("link", { name: "Download Windows companion" })).toHaveAttribute("href", "/public/downloads/MeshGraphLeapSetup.exe");
  await page.waitForFunction(() => document.querySelector("a-scene").components["gesture-controls"].running);
  expect(await page.evaluate(() => [window.leapLaunches, window.leapConnections])).toEqual([1, 2]);
  const installer = await page.request.get("/public/downloads/MeshGraphLeapSetup.exe");
  expect(installer.ok()).toBeTruthy();
  expect((await installer.body()).subarray(0, 2).toString()).toBe("MZ");
});
