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

test("IR pinch-drag pans in both axes without swipe, zoom, or selection", () => {
  const c = controls();
  c.frame([hand(1)]);
  c.frame([hand(1, "point", 0, 220, 0.9)]);
  c.frame([hand(1, "point", 30, 250, 0.9)]);
  assert.equal(c.gestures.length, 1);
  assert.equal(c.gestures[0].kind, "palm_pan");
  assert.ok(c.gestures[0].dx > 0 && c.gestures[0].dy < 0);
  assert.equal(c.gestures[0].viewRelative, true);
  c.frame([hand(1, "point", -30, 190, 0.9)]);
  assert.ok(c.gestures.at(-1).dx < 0 && c.gestures.at(-1).dy > 0);
  c.frame([hand(1)]);
  assert.ok(c.gestures.every((g) => g.kind === "palm_pan"));
  assert.ok(c.input.pointerValue);
});

test("small movements accumulate instead of disappearing below a frame threshold", () => {
  const c = controls();
  c.frame([hand(1)]);
  c.frame([hand(1, "point", 0, 220, 0.9)]);
  for (let i = 1; i <= 80; i++) c.frame([hand(1, "point", i * 0.15, 220, 0.9)]);
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

test("removing second hand restores focus; a fresh pinch starts pan", () => {
  const c = controls();
  c.frame([hand(1), hand(2, "open", 160)]);
  c.frame([hand(1, "open", 200, 350)]);
  assert.equal(c.input.hyperionMode, "idle");
  assert.ok(c.input.pointerValue);
  assert.equal(c.gestures.length, 0);
  c.frame([hand(1, "point", 200, 350, 0.9)]);
  assert.equal(c.input.hyperionMode, "pan");
  assert.equal(c.gestures.length, 0);
  c.frame([hand(1, "point", 230, 380, 0.9)]);
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
  assert.equal(c.gestures.length, 0);
  c.frame([hand(1, "point")]);
  c.input.hoveredInstanceId = 4;
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


test("open palm moves focus without transforming the graph", () => {
  const c = controls();
  c.frame([hand(1)]);
  const before = { ...c.input.pointerValue };
  c.frame([hand(1, "open", 70, 270)]);
  assert.equal(c.input.hyperionMode, "idle");
  assert.equal(c.gestures.length, 0);
  assert.ok(c.input.pointerValue.pixelX > before.pixelX);
  assert.ok(c.input.pointerValue.pixelY < before.pixelY);
});

test("pinch selects the focused node immediately and ignores finger curl", () => {
  const c = controls();
  c.frame([hand(1)]);
  c.input.hoveredInstanceId = 4;
  const focus = { ...c.input.pointerValue };
  c.frame([hand(1, "point", 0, 220, 0.9)]);
  assert.equal(c.gestures.length, 1);
  assert.equal(c.input.pointerValue.pixelX, focus.pixelX);
  c.frame([hand(1, "point", 2, 221, 0.9)]);
  c.frame([hand(1)]);
  assert.equal(c.gestures.length, 1);
  assert.equal(c.gestures[0].kind, "pinch");
  assert.equal(c.gestures[0].instanceId, 4);
});

test("tracking loss during a pinch cancels the click and drag", () => {
  const c = controls();
  c.frame([hand(1)]);
  c.frame([hand(1, "point", 0, 220, 0.9)]);
  c.frame([]);
  c.frame([hand(1)]);
  assert.equal(c.gestures.length, 0);
  assert.equal(c.input.hyperionPinchCandidate, null);
});


test("drag begins on the first deliberate movement without waiting for the filter", () => {
  const c = controls();
  c.frame([hand(1)]);
  c.frame([hand(1, "point", 0, 220, 0.9)]);
  c.frame([hand(1, "point", 4, 220, 0.9)], 8);
  assert.equal(c.input.hyperionPinchCandidate.dragging, true);
  assert.equal(c.gestures.at(-1).kind, "palm_pan");
});

test("finger curl and an incidental second hand cannot interrupt a held drag", () => {
  const c = controls();
  c.frame([hand(1)]);
  c.frame([hand(1, "point", 0, 220, 0.9)]);
  c.frame([hand(1, "point", 20, 220, 0.9)]);
  const candidate = c.input.hyperionPinchCandidate;
  c.frame([hand(1, "closed", 30, 240, 0.9), hand(2, "open", 120)]);
  assert.equal(c.input.hyperionMode, "pan");
  assert.equal(c.input.hyperionPinchCandidate, candidate);
  assert.equal(c.gestures.at(-1).kind, "palm_pan");
  c.frame([hand(1, "point", 40, 250, 0.4)]);
  assert.equal(c.input.hyperionMode, "pan");
  c.frame([hand(1)]);
  assert.equal(c.input.hyperionMode, "idle");
  assert.ok(c.gestures.every((g) => g.kind === "palm_pan"));
});

test("fast drag retains movement instead of clipping each tracking frame", () => {
  const c = controls();
  c.frame([hand(1)]);
  c.frame([hand(1, "point", 0, 220, 0.9)]);
  c.frame([hand(1, "point", 100, 220, 0.9)]);
  assert.ok(c.gestures.at(-1).dx > 0.06);
});

test("open-palm focus does not jump when fingertips curl or extend", () => {
  const c = controls();
  const open = hand(1);
  c.frame([open]);
  const before = { ...c.input.pointerValue };
  const flexed = hand(1);
  flexed.digits[1].tip = [170, 380, 0];
  c.frame([flexed]);
  assert.equal(c.input.pointerValue.pixelX, before.pixelX);
  assert.equal(c.input.pointerValue.pixelY, before.pixelY);
});


test("pan applies the entire raw delta immediately, including tiny movements", () => {
  const c = controls();
  c.frame([hand(1)]);
  c.frame([hand(1, "point", 0, 220, 0.9)]);
  c.frame([hand(1, "point", 0.1, 220.2, 0.9)], 8);
  assert.equal(c.gestures.at(-1).kind, "palm_pan");
  assert.ok(Math.abs(c.gestures.at(-1).dx - 0.1 / 320) < 1e-10);
  const count = c.gestures.length;
  c.frame([hand(1, "point", 0.1, 220.2, 0.9)]);
  assert.equal(c.gestures.length, count, "stopping the hand has no filter tail");
});

test("pinch distance can select when strength is below its entry threshold", () => {
  const c = controls();
  c.frame([hand(1)]);
  c.input.hoveredInstanceId = 4;
  c.frame([{ ...hand(1, "point", 0, 220, 0.4), pinchDistance: 12 }]);
  assert.equal(c.gestures.at(-1).kind, "pinch");
});


test("a normal curled hand moves focus exactly like an open palm", () => {
  const c = controls();
  const relaxed = (x, y = 220) => ({ ...hand(1, "point", x, y),
    grabStrength: 0.8, confidence: 0.2,
    digits: Array.from({ length: 5 }, () => ({ extended: false })) });
  c.frame([hand(1)]);
  const before = { ...c.input.pointerValue };
  c.frame([relaxed(50, 240)]);
  assert.equal(c.input.hyperionMode, "idle");
  assert.ok(c.input.pointerValue.pixelX > before.pixelX);
  assert.ok(c.input.pointerValue.pixelY < before.pixelY);
  assert.equal(c.gestures.length, 0);
  c.input.hoveredInstanceId = 4;
  c.frame([{ ...relaxed(50, 240), pinchStrength: 0.8 }]);
  assert.equal(c.gestures.at(-1).kind, "pinch");
  assert.equal(c.gestures.at(-1).instanceId, 4);
  c.frame([{ ...relaxed(60, 240), pinchStrength: 0.8 }]);
  assert.equal(c.gestures.length, 1);
  assert.equal(c.input.hyperionMode, "select");
});

test("two normal hands zoom without requiring extended fingers", () => {
  const c = controls();
  const relaxed = (id, x) => ({ ...hand(id, "point", x), grabStrength: 0.65,
    digits: Array.from({ length: 5 }, () => ({ extended: false })) });
  c.frame([relaxed(1, -80), relaxed(2, 80)]);
  c.frame([relaxed(1, -100), relaxed(2, 100)]);
  assert.equal(c.input.hyperionMode, "zoom");
  assert.equal(c.gestures.at(-1).kind, "spread");
});


test("a focused pinch never pans even after focus is lost while held", () => {
  const c = controls();
  c.frame([hand(1)]);
  c.input.hoveredInstanceId = 4;
  c.frame([hand(1, "point", 0, 220, 0.9)]);
  c.input.hoveredInstanceId = null;
  c.frame([hand(1, "point", 100, 280, 0.9)]);
  assert.equal(c.input.hyperionMode, "select");
  assert.equal(c.gestures.length, 1);
  assert.equal(c.gestures[0].kind, "pinch");
  c.frame([hand(1, "point", 100, 280)]);
  c.frame([hand(1, "point", 100, 280, 0.9)]);
  c.frame([hand(1, "point", 110, 290, 0.9)]);
  assert.equal(c.input.hyperionMode, "pan");
  assert.equal(c.gestures.at(-1).kind, "palm_pan");
});
