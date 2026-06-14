import { expect, test } from "@playwright/test";

async function loadWithWebXr(page, supported) {
  await page.addInitScript((isSupported) => {
    Object.defineProperty(navigator, "xr", {
      configurable: true,
      value: {
        addEventListener() {},
        isSessionSupported: async (mode) =>
          mode === "immersive-vr" ? isSupported : false,
      },
    });
  }, supported);
  await page.goto("/");
  await page.locator("a-scene").waitFor({ state: "attached" });
  await page.waitForFunction(() => document.querySelector("a-scene")?.hasLoaded);
}

test("resolves capability status in an unmocked secure browser", async ({
  page,
}) => {
  await page.goto("/");
  await page.locator("a-scene").waitFor({ state: "attached" });
  await page.waitForFunction(() => document.querySelector("a-scene")?.hasLoaded);

  expect(await page.evaluate(() => window.isSecureContext)).toBe(true);
  await expect(page.locator("#vr-status")).not.toHaveText("Checking WebXR...");
});

test("disables VR entry when immersive VR is unsupported", async ({ page }) => {
  await loadWithWebXr(page, false);

  await expect(page.locator("#enter-vr-button")).toBeDisabled();
  await expect(page.locator("#vr-status")).toHaveText("No VR headset detected");
});

test("enables VR entry when immersive VR is supported", async ({ page }) => {
  await loadWithWebXr(page, true);

  await expect(page.locator("#enter-vr-button")).toBeEnabled();
  await expect(page.locator("#vr-status")).toHaveText("Headset ready");
});

test("updates launcher state across successful VR entry and exit", async ({
  page,
}) => {
  await loadWithWebXr(page, true);
  await page.evaluate(() => {
    const scene = document.querySelector("a-scene");
    scene.enterVR = async () => scene.emit("enter-vr");
  });

  await page.locator("#enter-vr-button").click();
  await expect(page.locator("#enter-vr-button")).toHaveText("VR Active");
  await expect(page.locator("#enter-vr-button")).toBeDisabled();
  await expect(page.locator("#vr-status")).toHaveText(
    "Use headset controls to exit",
  );

  await page.evaluate(() => document.querySelector("a-scene").emit("exit-vr"));
  await expect(page.locator("#enter-vr-button")).toHaveText("Enter VR");
  await expect(page.locator("#enter-vr-button")).toBeEnabled();
  await expect(page.locator("#vr-status")).toHaveText("Headset ready");
});

test("recovers when VR entry fails", async ({ page }) => {
  await loadWithWebXr(page, true);
  await page.evaluate(() => {
    document.querySelector("a-scene").enterVR = async () => {
      throw new Error("permission denied");
    };
  });

  await page.locator("#enter-vr-button").click();
  await expect(page.locator("#enter-vr-button")).toBeEnabled();
  await expect(page.locator("#vr-status")).toHaveText(
    "Unable to start VR; check headset permissions",
  );
});

test("recovers when A-Frame leaves a rejected session request pending", async ({
  page,
}) => {
  await loadWithWebXr(page, true);
  await page.evaluate(() => {
    const scene = document.querySelector("a-scene");
    scene.components["webxr-launcher"].data.sessionTimeout = 25;
    scene.enterVR = () => new Promise(() => {});
  });

  await page.locator("#enter-vr-button").click();
  await expect(page.locator("#enter-vr-button")).toBeEnabled();
  await expect(page.locator("#vr-status")).toHaveText(
    "Unable to start VR; check headset permissions",
  );
});

test("configures WebXR features and controller ray interaction", async ({
  page,
}) => {
  await loadWithWebXr(page, true);

  const configuration = await page.evaluate(() => {
    const scene = document.querySelector("a-scene");
    return {
      webxr: scene.getAttribute("webxr"),
      vrUi: scene.getAttribute("vr-mode-ui"),
      controllers: [...document.querySelectorAll("[laser-controls]")].map(
        (controller) => ({
          laser: controller.getAttribute("laser-controls"),
          raycaster: controller.getAttribute("raycaster"),
          triggerEvents:
            controller.components["laser-controls"].config[
              "generic-tracked-controller-controls"
            ].cursor.downEvents,
        }),
      ),
    };
  });

  expect(configuration.webxr.optionalFeatures).toEqual(
    expect.arrayContaining(["local-floor", "bounded-floor", "hand-tracking"]),
  );
  expect(configuration.vrUi.enabled).toBe(false);
  expect(configuration.controllers).toHaveLength(2);
  expect(configuration.controllers.map(({ laser }) => laser.hand).sort()).toEqual(
    ["left", "right"],
  );
  for (const controller of configuration.controllers) {
    expect(controller.raycaster.objects).toBe(".clickable");
    expect(controller.triggerEvents).toContain("triggerdown");
  }
});

test("assembled rover explodes and dedicated control assembles it", async ({
  page,
}) => {
  await loadWithWebXr(page, false);
  await expect(page.locator("#assemble-rover-button")).toHaveCount(0);

  const state = await page.evaluate(() => {
    const assembled = document.querySelector("#assembled-rover");
    const exploded = document.querySelector("#graph-parts");
    assembled.emit("click");
    const afterExplode = {
      assembled: assembled.getAttribute("visible"),
      exploded: exploded.getAttribute("visible"),
    };
    return { afterExplode };
  });

  expect(state.afterExplode).toEqual({ assembled: false, exploded: true });
  await expect(page.locator("#assemble-rover-button")).toBeVisible();
  const horizontalOffset = await page.locator("#assemble-rover-button").evaluate(
    (button) => {
      const bounds = button.getBoundingClientRect();
      return bounds.left + bounds.width / 2 - window.innerWidth / 2;
    },
  );
  expect(horizontalOffset).toBeCloseTo(0);
  await page.locator("#assemble-rover-button").click();
  await expect(page.locator("#assemble-rover-button")).toHaveCount(0);

  const assembled = await page.evaluate(() => ({
    assembled: document.querySelector("#assembled-rover").getAttribute("visible"),
    exploded: document.querySelector("#graph-parts").getAttribute("visible"),
  }));
  expect(assembled).toEqual({ assembled: true, exploded: false });
});

test("selecting an exploded part opens the lower-left semantic panel", async ({
  page,
}) => {
  await page.route("**/api/v1/components/Drill", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        component_name: "Drill",
        display_name: "Alpha Proton X-Ray Spectrometer",
        category: "Science instrument",
        purpose: "Measures elemental composition of rocks and soil.",
        power_requirement: "Instrument measurement power",
        mission_note: "Analyzed Martian surface targets.",
      }),
    });
  });
  await loadWithWebXr(page, false);

  await page.evaluate(() => {
    document.querySelector("#assembled-rover").emit("click");
    document.querySelector("#ExplodedDrill").emit("click");
  });
  await expect(page.locator("#semantic-panel-title")).toHaveText(
    "Alpha Proton X-Ray Spectrometer",
  );

  const context = await page.evaluate(() => ({
    assembled: document.querySelector("#assembled-rover").getAttribute("visible"),
    exploded: document.querySelector("#graph-parts").getAttribute("visible"),
    panelPosition: getComputedStyle(
      document.querySelector("#semantic-panel"),
    ).position,
    panelLeft: getComputedStyle(document.querySelector("#semantic-panel")).left,
    panelBottom: getComputedStyle(document.querySelector("#semantic-panel"))
      .bottom,
    selected:
      document.querySelector("#ExplodedDrill").components["semantic-highlight"]
        .data.active,
    title: document.querySelector("#semantic-panel-title").textContent.trim(),
    details: document.querySelector("#semantic-panel-details").textContent,
  }));

  expect(context).toMatchObject({
    assembled: false,
    exploded: true,
    panelPosition: "fixed",
    panelLeft: "16px",
    panelBottom: "16px",
    selected: true,
    title: "Alpha Proton X-Ray Spectrometer",
  });
  expect(context.details).toContain("Measures elemental composition");
  expect(context.details).toContain("Analyzed Martian surface targets");

  await page.locator("#close-semantic-panel").click();
  await expect(page.locator("#semantic-panel")).toHaveCount(0);
  const selectionCleared = await page.evaluate(
    () =>
      document.querySelector("#ExplodedDrill").components["semantic-highlight"]
        .data.active,
  );
  expect(selectionCleared).toBe(false);
});

test("VR assemble control collapses rover and clears semantic context", async ({
  page,
}) => {
  await loadWithWebXr(page, false);

  const state = await page.evaluate(() => {
    const scene = document.querySelector("a-scene");
    document.querySelector("#assembled-rover").emit("click");
    document.querySelector("#ExplodedAntenna").emit("click");
    const control = document.querySelector("#vr-assemble-control");
    const desktopVisible = control.getAttribute("visible");
    scene.addState("vr-mode");
    scene.emit("enter-vr");
    const vrVisible = control.getAttribute("visible");
    document.querySelector("#vr-assemble-control").emit("click");
    return {
      desktopVisible,
      vrVisible,
      assembled: document.querySelector("#assembled-rover").getAttribute("visible"),
      exploded: document.querySelector("#graph-parts").getAttribute("visible"),
      semanticPanel: document.querySelector("#semantic-panel"),
      highlighted:
        document.querySelector("#ExplodedAntenna").components[
          "semantic-highlight"
        ].data.active,
    };
  });

  expect(state).toEqual({
    desktopVisible: false,
    vrVisible: true,
    assembled: true,
    exploded: false,
    semanticPanel: null,
    highlighted: false,
  });
});

test("starts from the elevated front three-quarter view", async ({ page }) => {
  await loadWithWebXr(page, false);

  const camera = await page.evaluate(() => {
    const element = document.querySelector("a-camera");
    const controls = element.components["look-controls"];
    return {
      position: element.getAttribute("position"),
      pitch: THREE.MathUtils.radToDeg(controls.pitchObject.rotation.x),
      yaw: THREE.MathUtils.radToDeg(controls.yawObject.rotation.y),
      fov: element.getAttribute("camera").fov,
    };
  });

  expect(camera).toMatchObject({
    position: { x: 5.5, y: 1.65, z: -3.5 },
    fov: 62,
  });
  expect(camera.pitch).toBeCloseTo(-15);
  expect(camera.yaw).toBeCloseTo(120);
});

test("major parts explode along their intended axes", async ({ page }) => {
  await loadWithWebXr(page, false);

  const targets = await page.evaluate(() => {
    document.querySelector("#assembled-rover").emit("click");
    return Object.fromEntries(
      [
        "ExplodedAntenna",
        "ExplodedLeftWheelAssembly",
        "ExplodedRightWheelAssembly",
        "ExplodedDrill",
      ].map((id) => [
        id,
        document.querySelector(`#${id}`).getAttribute("animation__position").to,
      ]),
    );
  });

  expect(targets).toEqual({
    ExplodedAntenna: "-2.6 -0.9 0",
    ExplodedLeftWheelAssembly: "0 0 1.3",
    ExplodedRightWheelAssembly: "0 0 -1.3",
    ExplodedDrill: "3.2 -0.4 0",
  });
});

test("wheel assemblies preserve their internal spacing and orientation", async ({
  page,
}) => {
  await loadWithWebXr(page, false);

  const assemblies = await page.evaluate(() =>
    ["Left", "Right"].map((side) => {
      const assembly = document.querySelector(
        `#Exploded${side}WheelAssembly`,
      );
      return {
        side,
        children: [...assembly.children].map((child) => ({
          id: child.id,
          position: child.getAttribute("position"),
          scale: child.getAttribute("scale"),
        })),
      };
    }),
  );

  expect(assemblies).toEqual([
    {
      side: "Left",
      children: [
        { id: "ExplodedLeftSuspension", position: { x: 0, y: -0.9, z: 0.9 }, scale: { x: 0.1, y: 0.1, z: 0.1 } },
        { id: "ExplodedWheelFL", position: { x: -1.4, y: -1, z: 1.5 }, scale: { x: 5, y: 5, z: 5 } },
        { id: "ExplodedWheelML", position: { x: 0, y: -1, z: 1.5 }, scale: { x: 5, y: 5, z: 5 } },
        { id: "ExplodedWheelRL", position: { x: 1.4, y: -1, z: 1.5 }, scale: { x: 5, y: 5, z: 5 } },
      ],
    },
    {
      side: "Right",
      children: [
        { id: "ExplodedRightSuspension", position: { x: 0, y: -0.9, z: -0.9 }, scale: { x: 0.1, y: 0.1, z: -0.1 } },
        { id: "ExplodedWheelFR", position: { x: -1.4, y: -1, z: -1.5 }, scale: { x: 5, y: 5, z: -5 } },
        { id: "ExplodedWheelMR", position: { x: 0, y: -1, z: -1.5 }, scale: { x: 5, y: 5, z: -5 } },
        { id: "ExplodedWheelRR", position: { x: 1.4, y: -1, z: -1.5 }, scale: { x: 5, y: 5, z: -5 } },
      ],
    },
  ]);
});

test("clicks outside visible rover geometry do not explode it", async ({
  page,
}) => {
  await loadWithWebXr(page, false);

  const state = await page.evaluate(() => {
    document.querySelector("a-plane").emit("click");
    document.querySelector("a-scene").emit("click");
    return {
      assembled: document
        .querySelector("#assembled-rover")
        .getAttribute("visible"),
      exploded: document.querySelector("#graph-parts").getAttribute("visible"),
      raycastTargets: document.querySelector("a-scene").getAttribute("raycaster")
        .objects,
    };
  });

  expect(state).toEqual({
    assembled: true,
    exploded: false,
    raycastTargets: ".clickable",
  });
});
