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
        color: graph.mode === "citations" ? 0x2d708e : 0x9b63ff,
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
      color.setHSL((hash(topic) % 360) / 360, 0.72, 0.58);
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
      this.nodeMesh.setColorAt(index, new THREE.Color(0xffffff));
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
      worker: { default: "/public/hand-worker.js" },
      graph: { type: "selector" },
      rig: { type: "selector" },
    },

    init() {
      this.running = false;
      this.framePending = false;
      this.raycaster = new THREE.Raycaster();
      this.pointer = new THREE.Vector2();
      this.onToggle = () => (this.running ? this.stop() : this.start());
      window.addEventListener("citation-gesture-toggle", this.onToggle);
    },

    async start() {
      try {
        emitMacro("citation-gesture-status", "Requesting camera access…");
        this.stream = await navigator.mediaDevices.getUserMedia({
          video: { facingMode: "user", width: 640, height: 480, frameRate: 30 },
          audio: false,
        });
        this.video = document.createElement("video");
        this.video.muted = true;
        this.video.playsInline = true;
        this.video.srcObject = this.stream;
        await this.video.play();

        this.workerInstance = new Worker(this.data.worker, { type: "module" });
        this.workerInstance.onmessage = (event) => this.onWorkerMessage(event.data);
        this.workerInstance.onerror = (event) => {
          console.error("MeshGraph hand worker failed", event);
          emitMacro("citation-gesture-status", "Hand tracking failed to initialize");
          this.stop();
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
        this.lastFrameAt = 0;
        emitMacro("citation-gesture-status", `Hand tracking active · ${this.transport}`);
        requestAnimationFrame((time) => this.captureFrame(time));
      } catch (error) {
        console.error("Unable to start gesture input", error);
        emitMacro("citation-gesture-status", "Camera permission or MediaPipe unavailable");
        this.stop();
      }
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
        emitMacro("citation-gesture-status", `Hand tracking active · ${this.transport}`);
        return;
      }
      if (message.type === "error") {
        emitMacro("citation-gesture-status", message.message);
        return;
      }
      if (message.type !== "result") return;
      this.framePending = false;
      if (message.landmarksBuffer) {
        // The transferable fallback remains on the JS side; raw landmarks never cross into WASM.
        this.latestLandmarks = new Float32Array(message.landmarksBuffer);
      }
      if (message.gesture) this.applyGesture(message.gesture);
    },

    applyGesture(gesture) {
      const graph = this.data.graph?.components["af-force-graph"];
      const graphObject = this.data.graph?.object3D;
      if (!graph || !graphObject) return;

      if (gesture.kind === "palm_drag") {
        graphObject.rotation.y -= gesture.dx * 2.4;
        graphObject.rotation.x = THREE.MathUtils.clamp(
          graphObject.rotation.x + gesture.dy * 1.5,
          -0.75,
          0.75,
        );
      } else if (gesture.kind === "pinch") {
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
      } else if (gesture.kind === "sweep") {
        graph.setYearCutoff(1 - gesture.x);
      }
    },

    stop() {
      this.running = false;
      this.framePending = false;
      this.workerInstance?.terminate();
      this.workerInstance = null;
      this.stream?.getTracks().forEach((track) => track.stop());
      this.stream = null;
      if (this.video) this.video.srcObject = null;
      this.video = null;
      emitMacro("citation-gesture-status", "Gesture camera is off");
    },

    remove() {
      window.removeEventListener("citation-gesture-toggle", this.onToggle);
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
