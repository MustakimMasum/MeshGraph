import fs from "node:fs/promises";
import path from "node:path";
import { chromium } from "@playwright/test";

const outputDirectory = path.resolve("demo");
await fs.mkdir(outputDirectory, { recursive: true });

const browser = await chromium.launch({ channel: "chrome", headless: true });
const context = await browser.newContext({
  viewport: { width: 1440, height: 900 },
  recordVideo: {
    dir: outputDirectory,
    size: { width: 1440, height: 900 },
  },
});
const page = await context.newPage();
const video = page.video();

async function pause(milliseconds) {
  await page.waitForTimeout(milliseconds);
}

async function setControl(label) {
  await page.evaluate((value) => {
    document.querySelector("#demo-control").textContent = value;
    document.querySelector("#demo-control").classList.toggle("is-active", Boolean(value));
  }, label);
}

async function rotateView() {
  await setControl("LEFT MOUSE  LOOK");
  await page.mouse.move(900, 450);
  await page.mouse.down();
  await page.mouse.move(960, 420, { steps: 40 });
  await page.mouse.up();
  await setControl("");
  await pause(900);
}

async function zoomView() {
  await setControl("SCROLL  ZOOM");
  await page.mouse.move(800, 500);
  await page.mouse.wheel(0, -360);
  await pause(900);
  await page.mouse.wheel(0, 180);
  await setControl("");
  await pause(900);
}

async function moveCursorTo(selector) {
  const point = await page.evaluate((target) => {
    const element = document.querySelector(target);
    const camera = document.querySelector("a-camera")?.getObject3D("camera");
    if (!element || !camera) return null;

    const position = new THREE.Vector3();
    element.object3D.getWorldPosition(position);
    position.project(camera);
    return {
      x: ((position.x + 1) / 2) * window.innerWidth,
      y: ((1 - position.y) / 2) * window.innerHeight,
    };
  }, selector);

  if (point) {
    await page.evaluate(({ x, y }) => {
      const cursor = document.querySelector("#demo-cursor");
      cursor.style.transform = `translate(${x}px, ${y}px)`;
    }, point);
    await pause(900);
  }
}

async function clickMesh(selector) {
  await moveCursorTo(selector);
  await page.evaluate((target) => {
    const cursor = document.querySelector("#demo-cursor");
    cursor.classList.remove("is-clicking");
    void cursor.offsetWidth;
    cursor.classList.add("is-clicking");
    document.querySelector(target)?.emit("click");
  }, selector);
  await pause(450);
}

await page.goto("http://127.0.0.1:3000");
await page.locator("a-scene").waitFor({ state: "attached" });
await page.waitForFunction(() => document.querySelector("a-scene")?.hasLoaded);
await page.evaluate(() => {
  const style = document.createElement("style");
  style.textContent = `
    #demo-cursor {
      position: fixed;
      z-index: 9999;
      top: -12px;
      left: -12px;
      width: 24px;
      height: 24px;
      pointer-events: none;
      border: 3px solid #00ff88;
      border-radius: 50%;
      background: rgba(0, 255, 136, 0.18);
      box-shadow: 0 0 0 2px rgba(7, 27, 24, 0.8), 0 0 18px rgba(0, 255, 136, 0.8);
      transform: translate(720px, 450px);
      transition: transform 800ms ease-in-out;
    }
    #demo-cursor::after {
      content: "";
      position: absolute;
      inset: 7px;
      border-radius: 50%;
      background: #00ff88;
    }
    #demo-cursor.is-clicking {
      animation: demo-cursor-click 450ms ease-out;
    }
    #demo-control {
      position: fixed;
      z-index: 9998;
      left: 50%;
      top: 1rem;
      min-width: 7rem;
      padding: 0.55rem 0.8rem;
      color: #ffffff;
      background: rgba(23, 50, 77, 0.88);
      border: 1px solid rgba(255, 255, 255, 0.5);
      border-radius: 0.5rem;
      font: 700 13px/1.2 monospace;
      text-align: center;
      opacity: 0;
      transform: translateX(-50%) translateY(-0.5rem);
      transition: opacity 180ms ease, transform 180ms ease;
      pointer-events: none;
    }
    #demo-control.is-active {
      opacity: 1;
      transform: translateX(-50%) translateY(0);
    }
    @keyframes demo-cursor-click {
      50% { scale: 1.75; background: rgba(0, 255, 136, 0.5); }
    }
  `;
  document.head.appendChild(style);
  const cursor = document.createElement("div");
  cursor.id = "demo-cursor";
  document.body.appendChild(cursor);
  const control = document.createElement("div");
  control.id = "demo-control";
  document.body.appendChild(control);
});

await pause(3000);
await rotateView();
await zoomView();
await pause(1000);
await clickMesh("#assembled-rover");
await pause(3500);
await clickMesh("#ExplodedDrill");
await page.locator("#semantic-panel-title").waitFor();
await pause(6000);
await clickMesh("#ExplodedWheelFR");
await page.locator("#semantic-panel-title").waitFor();
await pause(6000);
await rotateView();
await pause(1500);
await page.mouse.move(720, 840, { steps: 30 });
await page.evaluate(() => {
  const cursor = document.querySelector("#demo-cursor");
  cursor.style.transform = "translate(720px, 840px)";
});
await pause(900);
await page.locator("#assemble-rover-button").click();
await page.evaluate(() => document.querySelector("#demo-cursor").classList.add("is-clicking"));
await pause(4500);

await context.close();
await browser.close();

const generatedVideo = await video.path();
const destination = path.join(outputDirectory, "graphmesh-demo.webm");
await fs.rm(destination, { force: true });
await fs.rename(generatedVideo, destination);
console.log(destination);
