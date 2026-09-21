/* global AFRAME, THREE */

(() => {
  const emitMacro = (name, detail) => {
    window.dispatchEvent(
      new CustomEvent(name, {
        detail: typeof detail === "string" ? detail : JSON.stringify(detail),
      }),
    );
  };

  const hash = (value) => {
    let result = 2166136261;
    for (let index = 0; index < value.length; index += 1) {
      result ^= value.charCodeAt(index);
      result = Math.imul(result, 16777619);
    }
    return result >>> 0;
  };

  AFRAME.registerComponent("af-force-graph", {
    schema: {
      endpoint: { default: "/api/v1/citations/graph?mode=citations" },
      maxNodes: { default: 12000 },
    },

    init() {
      this.nodes = [];
      this.links = [];
      this.positions = new Float32Array();
      this.velocities = new Float32Array();
      this.yearRange = [new Date().getFullYear(), new Date().getFullYear()];
      this.selectedIndex = -1;
      this.frame = 0;
      this.matrix = new THREE.Matrix4();
      this.quaternion = new THREE.Quaternion();
      this.scale = new THREE.Vector3();
      this.onClick = (event) => {
        const instanceId = event.detail?.intersection?.instanceId;
        if (Number.isInteger(instanceId)) this.selectInstance(instanceId);
      };
      this.onReload = () => this.loadGraph();
      this.onClear = () => this.clearSelection();
      this.el.addEventListener("click", this.onClick);
      window.addEventListener("citation-graph-reload", this.onReload);
      window.addEventListener("citation-selection-clear", this.onClear);
      this.loadGraph();
    },

    update(oldData) {
      if (oldData.endpoint && oldData.endpoint !== this.data.endpoint) {
        this.loadGraph();
      }
    },

    async loadGraph() {
      this.abortController?.abort();
      this.abortController = new AbortController();
      try {
        const response = await fetch(this.data.endpoint, {
          signal: this.abortController.signal,
          headers: { Accept: "application/json" },
        });
        if (!response.ok) throw new Error(`Graph endpoint returned ${response.status}`);
        const graph = await response.json();
        this.buildGraph(graph);
        emitMacro("citation-graph-loaded", {
          nodes: this.nodes.length,
          links: this.links.length,
          mode: graph.mode,
        });
      } catch (error) {
        if (error.name !== "AbortError") {
          console.error("MeshGraph topology load failed", error);
          emitMacro("citation-gesture-status", "Graph service is unavailable");
        }
      }
    },

    buildGraph(graph) {
      this.disposeGraph();
      this.nodes = (graph.nodes || []).slice(0, this.data.maxNodes);
      const nodeIds = new Set(this.nodes.map((node) => node.id));
      this.links = (graph.links || []).filter(
        (link) => nodeIds.has(link.source) && nodeIds.has(link.target),
      );
      this.idToIndex = new Map(this.nodes.map((node, index) => [node.id, index]));
      this.positions = new Float32Array(this.nodes.length * 3);
      this.velocities = new Float32Array(this.nodes.length * 2);
      this.anchorPositions = new Float32Array(this.nodes.length * 2);
      this.visible = new Uint8Array(this.nodes.length).fill(1);

      const datedYears = this.nodes
        .map((node) => node.year)
        .filter((year) => Number.isFinite(year));
      const currentYear = new Date().getFullYear();
      const minYear = datedYears.length ? Math.min(...datedYears) : currentYear;
      const maxYear = datedYears.length ? Math.max(...datedYears) : currentYear;
      this.yearRange = [minYear, maxYear];

      const topics = [...new Set(this.nodes.map((node) => node.topic || "Unclassified"))];
      this.topicTargets = new Map(
        topics.map((topic, index) => {
          const angle = (index / Math.max(1, topics.length)) * Math.PI * 2;
          const radius = 5 + Math.sqrt(topics.length) * 0.8;
          return [topic, [Math.cos(angle) * radius, Math.sin(angle) * radius]];
        }),
      );

      const geometry = new THREE.IcosahedronGeometry(0.23, 1);
      const material = new THREE.MeshBasicMaterial({
        transparent: true,
        opacity: 0.96,
        toneMapped: false,
      });
      this.nodeMesh = new THREE.InstancedMesh(geometry, material, this.nodes.length);
      this.nodeMesh.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
      this.nodeMesh.frustumCulled = false;
      this.nodeMesh.userData.meshGraph = this;

      this.nodes.forEach((node, index) => {
        const topic = node.topic || "Unclassified";
        const [topicX, topicY] = this.topicTargets.get(topic);
        const seed = hash(node.id);
        const angle = (seed % 6283) / 1000;
        const radius = 0.8 + ((seed >>> 8) % 500) / 160;
        const offset = index * 3;
        this.positions[offset] = topicX + Math.cos(angle) * radius;
        this.positions[offset + 1] = topicY + Math.sin(angle) * radius;
        this.positions[offset + 2] = this.yearToZ(node.year);
        this.anchorPositions[index * 2] = this.positions[offset];
        this.anchorPositions[index * 2 + 1] = this.positions[offset + 1];
        this.nodeMesh.setColorAt(index, this.topicColor(topic));
      });
      material.needsUpdate = true;
      if (this.nodeMesh.instanceColor) {
        this.nodeMesh.instanceColor.setUsage(THREE.DynamicDrawUsage);
      }
      this.el.setObject3D("citation-nodes", this.nodeMesh);

      const linkPositions = new Float32Array(this.links.length * 6);
      const linkGeometry = new THREE.BufferGeometry();
      linkGeometry.setAttribute(
        "position",
        new THREE.BufferAttribute(linkPositions, 3).setUsage(THREE.DynamicDrawUsage),
      );
      const linkMaterial = new THREE.LineBasicMaterial({
        color: graph.mode === "citations" ? 0x54758e : 0x765ca8,
        transparent: true,
        opacity: 0.3,
        depthWrite: false,
      });
      this.linkLines = new THREE.LineSegments(linkGeometry, linkMaterial);
      this.linkLines.frustumCulled = false;
      this.el.setObject3D("citation-links", this.linkLines);
      this.updateInstances();
      this.updateLinks();
    },

    yearToZ(year) {
      if (!Number.isFinite(year)) return 0;
      const [minYear, maxYear] = this.yearRange;
      if (minYear === maxYear) return 0;
      return -12 + ((year - minYear) / (maxYear - minYear)) * 24;
    },

    topicColor(topic) {
      const color = new THREE.Color();
      color.setHSL((hash(topic) % 360) / 360, 0.68, 0.42);
      return color;
    },

    tick(_time, delta) {
      if (!this.nodeMesh || !this.nodes.length) return;
      const dt = Math.min(delta / 16.6667, 2);
      const linkStrength = 0.00015 * dt;

      for (const link of this.links) {
        const source = this.idToIndex.get(link.source);
        const target = this.idToIndex.get(link.target);
        if (source === undefined || target === undefined) continue;
        const sourceOffset = source * 3;
        const targetOffset = target * 3;
        const dx = this.positions[targetOffset] - this.positions[sourceOffset];
        const dy = this.positions[targetOffset + 1] - this.positions[sourceOffset + 1];
        const weight = Math.min(5, link.weight || 1);
        this.velocities[source * 2] += dx * linkStrength * weight;
        this.velocities[source * 2 + 1] += dy * linkStrength * weight;
        this.velocities[target * 2] -= dx * linkStrength * weight;
        this.velocities[target * 2 + 1] -= dy * linkStrength * weight;
      }

      this.nodes.forEach((node, index) => {
        const offset = index * 3;
        const velocityOffset = index * 2;
        const targetX = this.anchorPositions[velocityOffset];
        const targetY = this.anchorPositions[velocityOffset + 1];
        this.velocities[velocityOffset] +=
          (targetX - this.positions[offset]) * 0.008 * dt;
        this.velocities[velocityOffset + 1] +=
          (targetY - this.positions[offset + 1]) * 0.008 * dt;

        this.positions[offset] += this.velocities[velocityOffset] * dt;
        this.positions[offset + 1] += this.velocities[velocityOffset + 1] * dt;
        this.velocities[velocityOffset] *= 0.93;
        this.velocities[velocityOffset + 1] *= 0.93;
      });

      this.frame += 1;
      this.updateInstances();
      if (this.frame % 2 === 0) this.updateLinks();
    },

    updateInstances() {
      this.nodes.forEach((node, index) => {
        const offset = index * 3;
        const radius = 0.65 + Math.min(1.75, Math.log10((node.citationCount || 0) + 1) * 0.42);
        const visibleScale = this.visible[index] ? radius : 0;
        this.scale.setScalar(visibleScale);
        this.matrix.compose(
          new THREE.Vector3(
            this.positions[offset],
            this.positions[offset + 1],
            this.positions[offset + 2],
          ),
          this.quaternion,
          this.scale,
        );
        this.nodeMesh.setMatrixAt(index, this.matrix);
      });
      this.nodeMesh.instanceMatrix.needsUpdate = true;
    },

    updateLinks() {
      const attribute = this.linkLines?.geometry.attributes.position;
      if (!attribute) return;
      const output = attribute.array;
      this.links.forEach((link, index) => {
        const source = this.idToIndex.get(link.source);
        const target = this.idToIndex.get(link.target);
        const lineOffset = index * 6;
        if (
          source === undefined ||
          target === undefined ||
          !this.visible[source] ||
          !this.visible[target]
        ) {
          output.fill(0, lineOffset, lineOffset + 6);
          return;
        }
        const sourceOffset = source * 3;
        const targetOffset = target * 3;
        output.set(this.positions.subarray(sourceOffset, sourceOffset + 3), lineOffset);
        output.set(this.positions.subarray(targetOffset, targetOffset + 3), lineOffset + 3);
      });
      attribute.needsUpdate = true;
      this.linkLines.geometry.computeBoundingSphere();
    },

    selectInstance(index) {
      if (!this.nodes[index] || !this.visible[index]) return;
      this.clearSelection();
      this.selectedIndex = index;
      this.nodeMesh.setColorAt(index, new THREE.Color(0x17324d));
      this.nodeMesh.instanceColor.needsUpdate = true;
      emitMacro("citation-node-selected", this.nodes[index].id);
    },

    clearSelection() {
      if (this.selectedIndex < 0 || !this.nodeMesh) return;
      const node = this.nodes[this.selectedIndex];
      this.nodeMesh.setColorAt(this.selectedIndex, this.topicColor(node.topic || "Unclassified"));
      this.nodeMesh.instanceColor.needsUpdate = true;
      this.selectedIndex = -1;
    },

    setYearCutoff(normalizedX) {
      const [minYear, maxYear] = this.yearRange;
      const cutoff = Math.round(minYear + THREE.MathUtils.clamp(normalizedX, 0, 1) * (maxYear - minYear));
      this.nodes.forEach((node, index) => {
        this.visible[index] = !Number.isFinite(node.year) || node.year <= cutoff ? 1 : 0;
      });
      if (this.selectedIndex >= 0 && !this.visible[this.selectedIndex]) this.clearSelection();
      this.updateInstances();
      this.updateLinks();
      emitMacro("citation-year-filter", `${minYear} – ${cutoff}`);
    },

    disposeGraph() {
      for (const key of ["citation-nodes", "citation-links"]) {
        const object = this.el.getObject3D(key);
        object?.geometry?.dispose();
        object?.material?.dispose();
        if (object) this.el.removeObject3D(key);
      }
      this.nodeMesh = null;
      this.linkLines = null;
    },

    remove() {
      this.abortController?.abort();
      this.el.removeEventListener("click", this.onClick);
      window.removeEventListener("citation-graph-reload", this.onReload);
      window.removeEventListener("citation-selection-clear", this.onClear);
      this.disposeGraph();
    },
  });

  AFRAME.registerComponent("gesture-controls", {
    schema: {
      worker: { default: "/public/hand-worker.js?v=0.10.35-4" },
      hyperionUrl: { default: "ws://127.0.0.1:6437/hands" },
      graph: { type: "selector" },
      rig: { type: "selector" },
    },

    init() {
      this.running = false;
      this.starting = false;
      this.inputSource = "webcam";
      this.framePending = false;
      this.captureStarted = false;
      this.startToken = 0;
      this.handCount = -1;
      this.lastGestureKind = null;
      this.lastGestureStatusAt = 0;
      this.hoveredInstanceId = null;
      this.snappedNodeMesh = null;
      this.pinchLockUntil = 0;
      this.projectedNode = new THREE.Vector3();
      this.raycaster = new THREE.Raycaster();
      this.pointer = new THREE.Vector2();
      this.hyperionSocket = null;
      this.hyperionHands = new Map();
      this.hyperionPrimaryId = null;
      this.hyperionPinching = false;
      this.hyperionLastSpread = null;
      this.hyperionSwipeCooldownUntil = 0;
      this.onToggle = (event) => {
        const source = event.detail === "hyperion" ? "hyperion" : "webcam";
        if ((this.running || this.starting) && source === this.inputSource) this.stop();
        else {
          if (this.running || this.starting) this.stop();
          this.start(source);
        }
      };
      this.onSource = (event) => {
        const source = event.detail === "hyperion" ? "hyperion" : "webcam";
        if (source === this.inputSource) return;
        const wasActive = this.running || this.starting;
        if (wasActive) this.stop();
        this.inputSource = source;
        if (wasActive) this.start(source);
        else {
          const label = source === "hyperion" ? "Leap Motion · Hyperion" : "Webcam";
          emitMacro("citation-gesture-status", `Hand input is off · ${label} selected`);
        }
      };
      window.addEventListener("citation-gesture-toggle", this.onToggle);
      window.addEventListener("citation-gesture-source", this.onSource);
    },

    start(source = this.inputSource) {
      this.inputSource = source === "hyperion" ? "hyperion" : "webcam";
      if (this.inputSource === "hyperion") this.startHyperion();
      else this.startWebcam();
    },

    async startWebcam() {
      if (this.running || this.starting) return;
      this.starting = true;
      this.handCount = -1;
      const startToken = ++this.startToken;

      try {
        if (!window.isSecureContext || !navigator.mediaDevices?.getUserMedia) {
          throw new Error("Camera access requires HTTPS or localhost");
        }
        emitMacro("citation-gesture-status", "Requesting camera access…");
        const stream = await navigator.mediaDevices.getUserMedia({
          video: { facingMode: "user", width: 640, height: 480, frameRate: 30 },
          audio: false,
        });
        if (startToken !== this.startToken) {
          stream.getTracks().forEach((track) => track.stop());
          return;
        }
        this.stream = stream;
        this.video = document.createElement("video");
        this.video.muted = true;
        this.video.playsInline = true;
        this.video.srcObject = this.stream;
        await this.video.play();
        if (startToken !== this.startToken) {
          stream.getTracks().forEach((track) => track.stop());
          if (this.stream === stream) this.stream = null;
          return;
        }

        // Keep the worker bootstrap same-origin and classic. MediaPipe is loaded with
        // import() inside the worker, where failures can be reported instead of
        // surfacing as an opaque module-worker error in Chromium.
        const worker = new Worker(this.data.worker);
        this.workerInstance = worker;
        worker.onmessage = (event) => this.onWorkerMessage(event.data);
        worker.onerror = (event) => {
          if (this.workerInstance !== worker) return;
          const details = [event.message, event.filename, event.lineno]
            .filter(Boolean)
            .join(" · ");
          console.error("MeshGraph hand worker failed", details || "Unknown worker error");
          this.stop(details ? `Hand tracking failed: ${details}` : "Hand tracking failed to initialize");
        };

        if (crossOriginIsolated && typeof SharedArrayBuffer !== "undefined") {
          this.sharedLandmarks = new SharedArrayBuffer(8 + 126 * Float32Array.BYTES_PER_ELEMENT);
          this.workerInstance.postMessage({ type: "init", sharedBuffer: this.sharedLandmarks });
          this.transport = "shared memory";
        } else {
          this.workerInstance.postMessage({ type: "init" });
          this.transport = "transferable buffers";
        }
        this.running = true;
        this.starting = false;
        this.lastFrameAt = 0;
        emitMacro("citation-gesture-status", "Loading hand tracking…");
      } catch (error) {
        console.error("Unable to start gesture input", error);
        if (startToken !== this.startToken) return;
        const status =
          error?.name === "NotAllowedError"
            ? "Camera access was denied"
            : error?.name === "NotFoundError"
              ? "No camera was found"
              : error?.name === "NotReadableError"
                ? "Camera is already in use"
                : error?.message || "Camera or MediaPipe unavailable";
        this.stop(status);
      }
    },

    startHyperion() {
      if (this.running || this.starting) return;
      this.starting = true;
      this.handCount = -1;
      this.hyperionHands.clear();
      this.hyperionPrimaryId = null;
      this.hyperionPinching = false;
      this.hyperionLastSpread = null;
      const startToken = ++this.startToken;
      emitMacro("citation-gesture-status", "Connecting to the local Hyperion bridge…");

      try {
        const socket = new WebSocket(this.data.hyperionUrl);
        this.hyperionSocket = socket;
        socket.addEventListener("open", () => {
          if (startToken !== this.startToken) return;
          this.running = true;
          this.starting = false;
          emitMacro("citation-gesture-status", "Hyperion bridge connected · waiting for tracking");
        });
        socket.addEventListener("message", (event) => {
          if (startToken !== this.startToken) return;
          try {
            this.onHyperionMessage(JSON.parse(event.data));
          } catch (error) {
            console.warn("Ignoring invalid Hyperion bridge message", error);
          }
        });
        socket.addEventListener("error", () => {
          if (startToken !== this.startToken) return;
          emitMacro(
            "citation-gesture-status",
            "Hyperion bridge unavailable · run cargo run --bin hyperion_bridge",
          );
        });
        socket.addEventListener("close", () => {
          if (startToken !== this.startToken) return;
          this.running = false;
          this.starting = false;
          this.updatePointer(null);
          emitMacro("citation-gesture-status", "Hyperion bridge disconnected · toggle to retry");
        });
      } catch (error) {
        console.error("Unable to start Hyperion input", error);
        this.stop(error?.message || "Hyperion input failed to initialize");
      }
    },

    onHyperionMessage(message) {
      if (message?.type === "status") {
        emitMacro("citation-gesture-status", message.message || `Hyperion · ${message.state}`);
        return;
      }
      if (message?.type !== "frame" || !Array.isArray(message.hands)) return;
      this.applyHyperionFrame(message.hands);
    },

    applyHyperionFrame(hands) {
      if (hands.length !== this.handCount) {
        this.handCount = hands.length;
        const label = hands.length === 1 ? "hand" : "hands";
        emitMacro(
          "citation-gesture-status",
          hands.length
            ? `Hyperion active · ${hands.length} ${label} detected`
            : "Hyperion active · show a hand over the sensor",
        );
      }
      if (!hands.length) {
        this.hyperionHands.clear();
        this.hyperionPrimaryId = null;
        this.hyperionPinching = false;
        this.hyperionLastSpread = null;
        this.updatePointer(null);
        return;
      }

      const primary =
        hands.find((hand) => hand.id === this.hyperionPrimaryId) ||
        hands.find((hand) => hand.chirality === "right") ||
        hands[0];
      if (primary.id !== this.hyperionPrimaryId) {
        this.hyperionPrimaryId = primary.id;
        this.hyperionPinching = false;
      }

      const indexTip = primary.digits?.[1]?.tip || primary.palm?.stabilizedPosition;
      if (!Array.isArray(indexTip)) return;
      const screenX = THREE.MathUtils.clamp(0.5 + indexTip[0] / 400, 0, 1);
      const screenY = THREE.MathUtils.clamp(1 - (indexTip[1] - 100) / 350, 0, 1);
      const pixelX = screenX * window.innerWidth;
      const pixelY = screenY * window.innerHeight;
      const gesturePinch = (Number(primary.flags) & 2) !== 0;
      const pinchStrength = Number(primary.pinchStrength) || 0;
      const pinching = this.hyperionPinching
        ? gesturePinch || pinchStrength > 0.5
        : gesturePinch || pinchStrength > 0.72;
      this.updatePointer({ pixelX, pixelY, pinching });

      const previous = this.hyperionHands.get(primary.id);
      const palmPosition = primary.palm?.stabilizedPosition || primary.palm?.position;
      const extendedDigits = (primary.digits || []).filter((digit) => digit.extended).length;
      const openPalm = (Number(primary.grabStrength) || 0) < 0.25 && extendedDigits >= 4;
      if (previous?.openPalm && openPalm && Array.isArray(palmPosition)) {
        const dx = THREE.MathUtils.clamp((palmPosition[0] - previous.palm[0]) / 320, -0.08, 0.08);
        const dy = THREE.MathUtils.clamp((previous.palm[1] - palmPosition[1]) / 300, -0.08, 0.08);
        if (Math.abs(dx) + Math.abs(dy) > 0.001) {
          this.applyGesture({ kind: "palm_pan", dx, dy });
        }
      }

      if (pinching && !this.hyperionPinching) {
        this.applyGesture({ kind: "pinch", x: 1 - screenX, y: screenY });
      }
      this.hyperionPinching = pinching;

      const velocity = primary.palm?.velocity || [0, 0, 0];
      const now = performance.now();
      if (
        now >= this.hyperionSwipeCooldownUntil &&
        Math.abs(velocity[0]) > 900 &&
        Math.abs(velocity[0]) > Math.abs(velocity[1]) * 1.4
      ) {
        this.applyGesture({ kind: "swipe", direction: velocity[0] < 0 ? "left" : "right" });
        this.hyperionSwipeCooldownUntil = now + 650;
      }

      if (hands.length >= 2) {
        const first = hands[0].palm?.position;
        const second = hands[1].palm?.position;
        if (Array.isArray(first) && Array.isArray(second)) {
          const distance = Math.hypot(first[0] - second[0], first[1] - second[1], first[2] - second[2]);
          if (this.hyperionLastSpread !== null) {
            const delta = THREE.MathUtils.clamp((distance - this.hyperionLastSpread) / 300, -0.08, 0.08);
            if (Math.abs(delta) > 0.001) this.applyGesture({ kind: "spread", delta });
          }
          this.hyperionLastSpread = distance;
        }
      } else {
        this.hyperionLastSpread = null;
      }

      this.hyperionHands = new Map(
        hands.map((hand) => [
          hand.id,
          {
            palm: hand.palm?.stabilizedPosition || hand.palm?.position || [0, 0, 0],
            openPalm:
              (Number(hand.grabStrength) || 0) < 0.25 &&
              (hand.digits || []).filter((digit) => digit.extended).length >= 4,
          },
        ]),
      );
    },

    async captureFrame(time) {
      if (!this.running) return;
      if (!this.framePending && time - this.lastFrameAt >= 32 && this.video.readyState >= 2) {
        this.framePending = true;
        this.lastFrameAt = time;
        try {
          const bitmap = await createImageBitmap(this.video);
          this.workerInstance.postMessage({ type: "frame", frame: bitmap, timestamp: time }, [bitmap]);
        } catch (error) {
          this.framePending = false;
          console.warn("Skipping hand-tracking frame", error);
        }
      }
      requestAnimationFrame((nextTime) => this.captureFrame(nextTime));
    },

    onWorkerMessage(message) {
      if (message.type === "ready") {
        if (!this.running) return;
        emitMacro("citation-gesture-status", `Hand tracking active · ${this.transport}`);
        if (!this.captureStarted) {
          this.captureStarted = true;
          requestAnimationFrame((time) => this.captureFrame(time));
        }
        return;
      }
      if (message.type === "error") {
        if (message.fatal) this.stop(message.message);
        else emitMacro("citation-gesture-status", message.message);
        return;
      }
      if (message.type !== "result") return;
      this.framePending = false;
      this.updatePointer(message.pointer);
      if (Number.isInteger(message.handCount) && message.handCount !== this.handCount) {
        this.handCount = message.handCount;
        const label = message.handCount === 1 ? "hand" : "hands";
        emitMacro(
          "citation-gesture-status",
          message.handCount
            ? `Hand tracking active · ${message.handCount} ${label} detected`
            : "Hand tracking active · show a hand to the camera",
        );
      }
      if (message.landmarksBuffer) {
        // The transferable fallback remains on the JS side; raw landmarks never cross into WASM.
        this.latestLandmarks = new Float32Array(message.landmarksBuffer);
      }
      if (message.gesture) this.applyGesture(message.gesture);
    },

    graphContext() {
      const element = this.data.graph || this.el.querySelector("#citation-graph");
      return {
        element,
        component: element?.components?.["af-force-graph"],
        object: element?.object3D,
      };
    },

    projectNodeToScreen(graph, instanceId, camera, width, height) {
      const offset = instanceId * 3;
      if (
        !graph.positions ||
        offset + 2 >= graph.positions.length ||
        !graph.visible?.[instanceId]
      ) {
        return false;
      }

      this.projectedNode
        .set(
          graph.positions[offset],
          graph.positions[offset + 1],
          graph.positions[offset + 2],
        )
        .applyMatrix4(graph.nodeMesh.matrixWorld)
        .project(camera);
      if (this.projectedNode.z < -1 || this.projectedNode.z > 1) return false;

      this.projectedNode.set(
        (this.projectedNode.x * 0.5 + 0.5) * width,
        (-this.projectedNode.y * 0.5 + 0.5) * height,
        this.projectedNode.z,
      );
      return true;
    },

    nearestPointerNode(graph, camera, rawX, rawY, width, height) {
      this.pointer.set((rawX / width) * 2 - 1, -((rawY / height) * 2 - 1));
      this.raycaster.setFromCamera(this.pointer, camera);
      const directHit = this.raycaster.intersectObject(graph.nodeMesh, false)[0];
      if (
        directHit &&
        Number.isInteger(directHit.instanceId) &&
        graph.visible?.[directHit.instanceId]
      ) {
        return directHit.instanceId;
      }

      const snapRadiusSquared = 38 * 38;
      let nearest = null;
      let nearestDistanceSquared = snapRadiusSquared;
      for (let index = 0; index < graph.nodes.length; index += 1) {
        if (!this.projectNodeToScreen(graph, index, camera, width, height)) continue;
        const dx = this.projectedNode.x - rawX;
        const dy = this.projectedNode.y - rawY;
        const distanceSquared = dx * dx + dy * dy;
        if (distanceSquared < nearestDistanceSquared) {
          nearest = index;
          nearestDistanceSquared = distanceSquared;
        }
      }
      return nearest;
    },

    updatePointer(pointer) {
      const reticle = document.querySelector("#gesture-pointer");
      if (!reticle || !pointer) {
        if (reticle) reticle.hidden = true;
        this.hoveredInstanceId = null;
        return;
      }

      const width = window.innerWidth;
      const height = window.innerHeight;
      const hasPixelPosition =
        Number.isFinite(pointer.pixelX) && Number.isFinite(pointer.pixelY);
      const rawX = hasPixelPosition
        ? THREE.MathUtils.clamp(pointer.pixelX, 0, width)
        : THREE.MathUtils.clamp(1 - pointer.x, 0, 1) * width;
      const rawY = hasPixelPosition
        ? THREE.MathUtils.clamp(pointer.pixelY, 0, height)
        : THREE.MathUtils.clamp(pointer.y, 0, 1) * height;
      reticle.hidden = false;
      reticle.classList.toggle("pinching", Boolean(pointer.pinching));

      const { component: graph } = this.graphContext();
      const camera = this.el.camera;
      let displayX = rawX;
      let displayY = rawY;
      if (graph?.nodeMesh && camera) {
        graph.nodeMesh.updateWorldMatrix(true, false);
        camera.updateWorldMatrix(true, false);

        if (this.snappedNodeMesh !== graph.nodeMesh) {
          this.hoveredInstanceId = null;
          this.snappedNodeMesh = graph.nodeMesh;
        }

        if (
          Number.isInteger(this.hoveredInstanceId) &&
          this.projectNodeToScreen(graph, this.hoveredInstanceId, camera, width, height)
        ) {
          const dx = this.projectedNode.x - rawX;
          const dy = this.projectedNode.y - rawY;
          const withinReleaseRadius = dx * dx + dy * dy <= 70 * 70;
          if (withinReleaseRadius || pointer.pinching || performance.now() < this.pinchLockUntil) {
            displayX = this.projectedNode.x;
            displayY = this.projectedNode.y;
          } else {
            this.hoveredInstanceId = null;
          }
        } else {
          this.hoveredInstanceId = null;
        }

        if (!Number.isInteger(this.hoveredInstanceId)) {
          this.hoveredInstanceId = this.nearestPointerNode(
            graph,
            camera,
            rawX,
            rawY,
            width,
            height,
          );
          if (
            Number.isInteger(this.hoveredInstanceId) &&
            this.projectNodeToScreen(graph, this.hoveredInstanceId, camera, width, height)
          ) {
            displayX = this.projectedNode.x;
            displayY = this.projectedNode.y;
          }
        }
      } else {
        this.hoveredInstanceId = null;
      }

      reticle.style.left = `${displayX}px`;
      reticle.style.top = `${displayY}px`;
      reticle.classList.toggle("hovering", Number.isInteger(this.hoveredInstanceId));
    },

    applyGesture(gesture) {
      // Leptos may attach this scene component before its child entities exist,
      // causing A-Frame's selector schema to resolve to null. Resolve lazily once
      // the graph is present instead of silently discarding every gesture.
      const { component: graph, object: graphObject } = this.graphContext();
      if (!graph || !graphObject) {
        emitMacro("citation-gesture-status", "Hand detected · graph controls unavailable");
        return;
      }

      const labels = {
        palm_pan: "Open-palm pan",
        pinch: "Pinch select",
        spread: "Two-hand zoom",
        swipe: "Hand swipe rotate",
      };
      const now = performance.now();
      if (gesture.kind !== this.lastGestureKind || now - this.lastGestureStatusAt > 500) {
        emitMacro("citation-gesture-status", labels[gesture.kind] || "Gesture recognized");
        this.lastGestureKind = gesture.kind;
        this.lastGestureStatusAt = now;
      }

      if (gesture.kind === "palm_pan") {
        graphObject.position.x = THREE.MathUtils.clamp(
          graphObject.position.x + gesture.dx * 12,
          -12,
          12,
        );
        graphObject.position.y = THREE.MathUtils.clamp(
          graphObject.position.y - gesture.dy * 10,
          -7,
          10,
        );
      } else if (gesture.kind === "pinch") {
        if (Number.isInteger(this.hoveredInstanceId)) {
          this.pinchLockUntil = performance.now() + 400;
          graph.selectInstance(this.hoveredInstanceId);
          return;
        }
        const camera = this.el.camera;
        if (!camera || !graph.nodeMesh) return;
        this.pointer.set((1 - gesture.x) * 2 - 1, -(gesture.y * 2 - 1));
        this.raycaster.setFromCamera(this.pointer, camera);
        const hit = this.raycaster.intersectObject(graph.nodeMesh, false)[0];
        if (hit && Number.isInteger(hit.instanceId)) graph.selectInstance(hit.instanceId);
      } else if (gesture.kind === "spread") {
        const factor = THREE.MathUtils.clamp(1 + gesture.delta * 1.8, 0.88, 1.12);
        graphObject.scale.multiplyScalar(factor);
        graphObject.scale.clampScalar(0.35, 3.5);
      } else if (gesture.kind === "swipe") {
        graphObject.rotation.y += gesture.direction === "left" ? -0.45 : 0.45;
      }
    },

    stop(status) {
      this.startToken += 1;
      this.running = false;
      this.starting = false;
      this.framePending = false;
      this.captureStarted = false;
      this.handCount = 0;
      this.lastGestureKind = null;
      this.snappedNodeMesh = null;
      this.pinchLockUntil = 0;
      this.hyperionHands.clear();
      this.hyperionPrimaryId = null;
      this.hyperionPinching = false;
      this.hyperionLastSpread = null;
      if (this.hyperionSocket) {
        this.hyperionSocket.close();
        this.hyperionSocket = null;
      }
      this.updatePointer(null);
      this.workerInstance?.terminate();
      this.workerInstance = null;
      this.stream?.getTracks().forEach((track) => track.stop());
      this.stream = null;
      if (this.video) this.video.srcObject = null;
      this.video = null;
      const defaultStatus =
        this.inputSource === "hyperion"
          ? "Hand input is off · Leap Motion · Hyperion selected"
          : "Hand input is off · Webcam selected";
      emitMacro("citation-gesture-status", status || defaultStatus);
    },

    remove() {
      window.removeEventListener("citation-gesture-toggle", this.onToggle);
      window.removeEventListener("citation-gesture-source", this.onSource);
      this.stop();
    },
  });

  AFRAME.registerComponent("vr-locomotion", {
    schema: { speed: { default: 3 }, deadzone: { default: 0.2 } },
    init() {
      this.axes = [0, 0];
      this.onAxis = (event) => {
        this.axes = [event.detail.x || 0, event.detail.y || 0];
      };
      this.bindController = () => {
        this.controller = this.el.querySelector("#left-controller");
        this.controller?.addEventListener("thumbstickmoved", this.onAxis);
      };
      if (this.el.sceneEl.hasLoaded) this.bindController();
      else this.el.sceneEl.addEventListener("loaded", this.bindController, { once: true });
      this.direction = new THREE.Vector3();
      this.right = new THREE.Vector3();
    },
    tick(_time, delta) {
      if (!this.el.sceneEl.is("vr-mode")) return;
      const [x, y] = this.axes;
      if (Math.abs(x) < this.data.deadzone && Math.abs(y) < this.data.deadzone) return;
      const camera = this.el.querySelector("[camera]")?.object3D;
      if (!camera) return;
      camera.getWorldDirection(this.direction);
      this.direction.y = 0;
      this.direction.normalize();
      this.right.crossVectors(this.direction, camera.up).normalize();
      const step = (delta / 1000) * this.data.speed;
      this.el.object3D.position.addScaledVector(this.direction, -y * step);
      this.el.object3D.position.addScaledVector(this.right, x * step);
    },
    remove() {
      this.el.sceneEl.removeEventListener("loaded", this.bindController);
      this.controller?.removeEventListener("thumbstickmoved", this.onAxis);
    },
  });
})();
