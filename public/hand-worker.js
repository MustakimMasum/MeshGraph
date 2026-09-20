const MEDIAPIPE_VERSION = "0.10.35";
const MEDIAPIPE_MODULE_URL =
  `https://cdn.jsdelivr.net/npm/@mediapipe/tasks-vision@${MEDIAPIPE_VERSION}/vision_bundle.mjs`;

const MAX_FLOATS = 126;
let handLandmarker;
let sharedMeta;
let sharedLandmarks;
let initialization;
let previousPalm;
let previousSpread;
let previousIndex;
let pinchActive = false;
let lastSweepAt = 0;

const distance = (left, right) =>
  Math.hypot(left.x - right.x, left.y - right.y, (left.z || 0) - (right.z || 0));

async function initialize(sharedBuffer) {
  if (sharedBuffer) {
    sharedMeta = new Int32Array(sharedBuffer, 0, 2);
    sharedLandmarks = new Float32Array(sharedBuffer, 8, MAX_FLOATS);
  }
  if (!initialization) {
    initialization = (async () => {
      const { FilesetResolver, HandLandmarker } = await import(MEDIAPIPE_MODULE_URL);
      const files = await FilesetResolver.forVisionTasks(
        `https://cdn.jsdelivr.net/npm/@mediapipe/tasks-vision@${MEDIAPIPE_VERSION}/wasm`,
      );
      handLandmarker = await HandLandmarker.createFromOptions(files, {
        baseOptions: {
          modelAssetPath:
            "https://storage.googleapis.com/mediapipe-models/hand_landmarker/hand_landmarker/float16/1/hand_landmarker.task",
          delegate: "CPU",
        },
        runningMode: "VIDEO",
        numHands: 2,
        minHandDetectionConfidence: 0.55,
        minTrackingConfidence: 0.5,
      });
    })();
  }
  await initialization;
  postMessage({ type: "ready" });
}

function flattenLandmarks(hands) {
  const output = new Float32Array(MAX_FLOATS);
  output.fill(Number.NaN);
  let cursor = 0;
  for (const hand of hands.slice(0, 2)) {
    for (const point of hand) {
      output[cursor++] = point.x;
      output[cursor++] = point.y;
      output[cursor++] = point.z;
    }
  }
  return output;
}

function classifyGesture(hands, timestamp) {
  if (hands.length === 2) {
    const first = hands[0][9];
    const second = hands[1][9];
    const spread = distance(first, second);
    const delta = previousSpread === undefined ? 0 : spread - previousSpread;
    previousSpread = spread;
    previousPalm = undefined;
    if (Math.abs(delta) > 0.005) return { kind: "spread", delta };
    return null;
  }

  previousSpread = undefined;
  if (hands.length !== 1) {
    previousPalm = undefined;
    previousIndex = undefined;
    pinchActive = false;
    return null;
  }

  const hand = hands[0];
  const thumbTip = hand[4];
  const indexTip = hand[8];
  const wrist = hand[0];
  const palmWidth = Math.max(distance(hand[5], hand[17]), 0.04);
  const pinch = distance(thumbTip, indexTip) / palmWidth < 0.42;
  if (pinch && !pinchActive) {
    pinchActive = true;
    previousPalm = undefined;
    return {
      kind: "pinch",
      x: (thumbTip.x + indexTip.x) / 2,
      y: (thumbTip.y + indexTip.y) / 2,
    };
  }
  if (!pinch) pinchActive = false;

  const extended = [8, 12, 16, 20].filter(
    (tip) => distance(hand[tip], wrist) > distance(hand[tip - 2], wrist) * 1.12,
  ).length;
  if (extended >= 4) {
    const currentPalm = { x: hand[9].x, y: hand[9].y };
    const gesture = previousPalm
      ? {
          kind: "palm_drag",
          dx: currentPalm.x - previousPalm.x,
          dy: currentPalm.y - previousPalm.y,
        }
      : null;
    previousPalm = currentPalm;
    previousIndex = undefined;
    return gesture;
  }
  previousPalm = undefined;

  const indexExtended =
    distance(indexTip, wrist) > distance(hand[6], wrist) * 1.18 && extended <= 2;
  if (indexExtended) {
    const current = { x: indexTip.x, timestamp };
    if (previousIndex) {
      const elapsed = Math.max(1, timestamp - previousIndex.timestamp);
      const velocity = Math.abs(current.x - previousIndex.x) / elapsed;
      if (velocity > 0.00075 && timestamp - lastSweepAt > 180) {
        lastSweepAt = timestamp;
        previousIndex = current;
        return { kind: "sweep", x: indexTip.x };
      }
    }
    previousIndex = current;
  } else {
    previousIndex = undefined;
  }
  return null;
}

self.onmessage = async (event) => {
  if (event.data.type === "init") {
    try {
      await initialize(event.data.sharedBuffer);
    } catch (error) {
      postMessage({
        type: "error",
        fatal: true,
        message: `MediaPipe initialization failed: ${error.message || error}`,
      });
    }
    return;
  }

  if (event.data.type !== "frame") return;
  const { frame, timestamp } = event.data;
  if (!handLandmarker) {
    frame.close();
    postMessage({ type: "result", gesture: null });
    return;
  }

  try {
    const result = handLandmarker.detectForVideo(frame, timestamp);
    const hands = result.landmarks || [];
    const flattened = flattenLandmarks(hands);
    const gesture = classifyGesture(hands, timestamp);
    frame.close();

    if (sharedLandmarks) {
      sharedLandmarks.set(flattened);
      Atomics.store(sharedMeta, 1, hands.length);
      Atomics.add(sharedMeta, 0, 1);
      postMessage({ type: "result", gesture, handCount: hands.length });
    } else {
      postMessage(
        {
          type: "result",
          gesture,
          handCount: hands.length,
          landmarksBuffer: flattened.buffer,
        },
        [flattened.buffer],
      );
    }
  } catch (error) {
    frame.close();
    postMessage({ type: "error", message: `Hand tracking frame failed: ${error.message || error}` });
    postMessage({ type: "result", gesture: null });
  }
};
