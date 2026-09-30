import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

const source = readFileSync(new URL("../public/constellation.js", import.meta.url), "utf8");

function controls() {
  let now = 0;
  const components = {};
  const events = [];
  const gestures = [];
  const sandbox = {
    AFRAME: { registerComponent: (name, definition) => { components[name] = definition; } },
    THREE: { MathUtils: { clamp: (v, min, max) => Math.min(max, Math.max(min, v)) } },
    performance: { now: () => now },
    window: { innerWidth: 1200, innerHeight: 800, dispatchEvent: (event) => events.push(event) },
    CustomEvent: class { constructor(type, options) { this.type = type; this.detail = options.detail; } },
  };
  vm.runInNewContext(source, sandbox);
  const input = Object.assign(Object.create(components["gesture-controls"]), {
    inputSource: "hyperion", hyperionHands: new Map(), hyperionPrimaryId: null,
    hyperionPinching: false, handCount: -1, updatePointer(pointer) { this.pointerValue = pointer; },
    applyGesture: (gesture) => gestures.push(gesture),
  });
  input.resetHyperionNavigation();
  return { input, gestures, events,
    frame(hands, dt = 16) { now += dt; input.applyHyperionFrame(hands); },
    advance(dt) { now += dt; input.tick(); },
  };
}

function hand(id, pose = "open", x = 0, y = 220, pinchStrength = 0) {
  const fingers = pose === "open" ? 5 : pose === "point" ? 2 : 0;
  return {
    id, chirality: id === 1 ? "right" : "left", confidence: 1, pinchStrength,
    grabStrength: pose === "closed" ? 0.95 : 0,
    palm: { position: [x, y, 0], stabilizedPosition: [0, 220, 0], velocity: [1200, 0, 0] },
    digits: Array.from({ length: 5 }, (_, i) => ({ extended: i < fingers, tip: [x, y + 30, 0] })),
  };
}

test("IR pan follows raw palm movement in both axes without swipe or zoom", () => {
  const c = controls();
  c.frame([hand(1)]);
  c.frame([hand(1, "open", 30, 250)]);
  assert.equal(c.gestures.length, 1);
  assert.equal(c.gestures[0].kind, "palm_pan");
  assert.ok(c.gestures[0].dx > 0 && c.gestures[0].dy < 0);
  assert.equal(c.gestures[0].viewRelative, true);
  c.frame([hand(1, "open", -30, 190)]);
  assert.ok(c.gestures.at(-1).dx < 0 && c.gestures.at(-1).dy > 0);
});

test("small movements accumulate instead of disappearing below a frame threshold", () => {
  const c = controls();
  c.frame([hand(1)]);
  for (let i = 1; i <= 30; i++) c.frame([hand(1, "open", i * 0.15)]);
  assert.ok(c.gestures.length > 0);
  assert.ok(c.gestures.every((g) => g.kind === "palm_pan" && g.dx > 0));
});

test("closed hand orbits continuously in both axes and releases immediately", () => {
  const c = controls();
  c.frame([hand(1, "closed")]);
  c.frame([hand(1, "closed", 25, 245)]);
  assert.equal(c.gestures.at(-1).kind, "orbit");
  assert.ok(c.gestures.at(-1).yaw > 0 && c.gestures.at(-1).pitch < 0);
  const count = c.gestures.length;
  c.frame([hand(1, "point", 150, 350)]);
  assert.equal(c.input.hyperionMode, "idle");
  assert.equal(c.gestures.length, count);
});

test("only two open palms zoom; relaxing either hand releases without another transform", () => {
  for (const releaseId of [1, 2]) {
    const c = controls();
    c.frame([hand(1, "open", -80), hand(2, "open", 80)]);
    c.frame([hand(1, "open", -110), hand(2, "open", 110)]);
    assert.equal(c.gestures.length, 1);
    assert.equal(c.gestures[0].kind, "spread");
    assert.ok(c.gestures[0].delta > 0);
    c.frame([hand(1, releaseId === 1 ? "closed" : "open", -160),
      hand(2, releaseId === 2 ? "closed" : "open", 160)]);
    assert.equal(c.input.hyperionMode, "idle");
    assert.equal(c.gestures.length, 1);
    c.frame([hand(1, "open", -250), hand(2, "open", 250)]);
    assert.equal(c.gestures.length, 1, "zoom re-entry must establish a fresh baseline");
    c.frame([hand(2, "open", 200), hand(1, "open", -200)]);
    assert.ok(c.gestures.at(-1).delta < 0, "hand array reordering preserves the baseline");
  }
});

test("removing second hand starts pan from the current position", () => {
  const c = controls();
  c.frame([hand(1), hand(2, "open", 160)]);
  c.frame([hand(1, "open", 200, 350)]);
  assert.equal(c.input.hyperionMode, "pan");
  assert.equal(c.gestures.length, 0);
  c.frame([hand(1, "open", 210, 360)]);
  assert.equal(c.gestures.at(-1).kind, "palm_pan");
});

test("pinch releasing navigation requires a release before selection", () => {
  const c = controls();
  c.frame([hand(1), hand(2, "open", 160)]);
  c.frame([hand(1, "point", 0, 220, 0.9), hand(2, "open", 160)]);
  assert.equal(c.gestures.length, 0);
  c.frame([hand(1, "point", 0, 220, 0.9)]);
  assert.equal(c.gestures.length, 0);
  c.frame([hand(1, "point")]);
  c.frame([hand(1, "point", 0, 220, 0.9)]);
  assert.equal(c.gestures.at(-1).kind, "pinch");
  assert.ok(c.input.pointerValue);
});

test("tracking gaps, new IDs, and invalid frames reset movement baselines", () => {
  const c = controls();
  c.frame([hand(1)]);
  c.frame([hand(1, "open", 200)], 250);
  assert.equal(c.gestures.length, 0);
  c.frame([hand(7, "open", -200)]);
  assert.equal(c.gestures.length, 0);
  c.frame([{ ...hand(7), confidence: 0.1 }]);
  assert.equal(c.input.hyperionMode, "idle");
  c.frame([{ ...hand(7), palm: { position: [NaN, 200, 0] } }]);
  assert.equal(c.gestures.length, 0);
  c.frame([hand(7)]);
  c.advance(200);
  assert.equal(c.input.hyperionMode, "idle");
  assert.equal(c.input.pointerValue, null);
  c.frame([hand(7, "open", 300)]);
  assert.equal(c.gestures.length, 0);
});

test("status/disconnection messages release navigation", () => {
  const c = controls();
  c.frame([hand(1, "closed")]);
  c.input.onHyperionMessage({ type: "status", state: "disconnected", message: "Camera disconnected" });
  assert.equal(c.input.hyperionMode, "idle");
  assert.equal(c.input.hyperionHands.size, 0);
  assert.equal(c.input.pointerValue, null);
});


test("a fist can orbit even when its curled fingers report strong pinch strength", () => {
  const c = controls();
  c.frame([hand(1, "closed", 0, 220, 0.95)]);
  c.frame([hand(1, "closed", 30, 250, 0.95)]);
  assert.equal(c.input.hyperionMode, "orbit");
  assert.equal(c.gestures.at(-1).kind, "orbit");
  assert.ok(c.gestures.every((g) => g.kind !== "pinch"));
});
