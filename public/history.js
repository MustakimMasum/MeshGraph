/* global AFRAME, THREE */
(() => {
  let graph = null;
  let revision = 0;
  let detail = null;
  let restoreCamera = null;
  const notify = (name, value) => window.dispatchEvent(new CustomEvent(name, {
    detail: typeof value === "string" ? value : JSON.stringify(value),
  }));
  const parse = event => {
    try { return JSON.parse(event.detail); } catch { return null; }
  };
  window.addEventListener("history-render", event => { graph = parse(event); revision++; });
  window.addEventListener("history-detail", event => { detail = parse(event); });
  window.addEventListener("history-camera-restore", event => { restoreCamera = parse(event); });
  const key = "meshgraph.jpl.views.v1";
  const readViews = () => {
    const stored = JSON.parse(localStorage.getItem(key) || "[]");
    return Array.isArray(stored) ? stored.filter(v => v.schema === 1 && v.view && typeof v.id === "string") : [];
  };
  const transform = element => element ? {
    position: element.object3D.position.toArray(),
    quaternion: element.object3D.quaternion.toArray(),
    scale: element.object3D.scale.toArray(),
  } : null;
  window.addEventListener("history-bookmark-command", event => {
    const command = parse(event);
    if (!command) return;
    try {
      let items = readViews();
      const selected = items.find(item => item.id === command.id);
      if (command.action === "save") {
        items.push({ schema: 1, id: crypto.randomUUID(), name: String(command.name || "JPL view").slice(0, 100),
          version: command.version, view: command.view,
          camera: { graph: transform(document.getElementById("citation-graph")), rig: transform(document.getElementById("rig")), camera: transform(document.getElementById("research-camera")) },
        });
      }
      if (command.action === "restore" && selected) notify("history-restore", selected);
      if (command.action === "delete") items = items.filter(item => item.id !== command.id);
      if (command.action === "rename" && selected) selected.name = String(command.name || "JPL view").slice(0, 100);
      if (["save", "delete", "rename"].includes(command.action)) localStorage.setItem(key, JSON.stringify(items));
      notify("history-bookmarks", items);
    } catch (error) { notify("citation-gesture-status", `Saved viewpoints unavailable: ${error.message}`); }
  });

  AFRAME.registerComponent("history-bridge", {
    init() {
      this.revision = -1;
      this.page = 0;
      this.lastDetail = null;
      this.labels = document.createElement("div");
      this.labels.id = "history-labels";
      document.body.appendChild(this.labels);
      this.labelItems = [];
      this.onEnter = () => { this.card?.setAttribute("visible", true); document.body.classList.add("history-immersive"); };
      this.onExit = () => { this.card?.setAttribute("visible", false); document.body.classList.remove("history-immersive"); };
      this.el.addEventListener("enter-vr", this.onEnter);
      this.el.addEventListener("exit-vr", this.onExit);
    },
    makeCard() {
      const camera = document.getElementById("research-camera");
      if (!camera) return;
      this.card = document.createElement("a-entity");
      this.card.id = "history-xr-card";
      this.card.setAttribute("position", "0 -0.18 -1.7");
      this.card.setAttribute("visible", false);
      const background = document.createElement("a-plane");
      background.setAttribute("width", "1.45");
      background.setAttribute("height", "1.3");
      background.setAttribute("material", "color: #eef4f8; opacity: 0.96; shader: flat; side: double");
      this.card.appendChild(background);
      this.text = document.createElement("a-entity");
      this.text.setAttribute("position", "-0.65 0.43 0.02");
      this.text.setAttribute("text", "value: Select a topic; color: #17324d; width: 1.3; wrapCount: 55; anchor: left; baseline: top");
      this.card.appendChild(this.text);
      [
        ["Home", () => notify("history-xr-action", "home")],
        ["Expand", () => notify("history-xr-action", "expand")],
        ["Focus", () => document.getElementById("citation-graph")?.components?.["af-force-graph"]?.focusSelected()],
        ["Connection", () => notify("history-xr-action", "connection")],
        ["Topic", () => notify("history-xr-action", "topic")],
        ["Next page", () => { this.page++; this.updateDetail(); }],
      ].forEach(([label, action], i) => {
        const button = document.createElement("a-plane");
        button.classList.add("history-xr-button");
        button.setAttribute("data-action", label);
        button.setAttribute("position", `${-0.46 + (i % 3) * 0.46} ${-0.4 - Math.floor(i / 3) * 0.16} 0.03`);
        button.setAttribute("width", "0.42");
        button.setAttribute("height", "0.12");
        button.setAttribute("material", "color: #244d63; shader: flat");
        button.setAttribute("text", { value: label, align: "center", width: 0.85, color: "#fff" });
        button.addEventListener("click", action);
        this.card.appendChild(button);
      });
      camera.appendChild(this.card);
    },
    updateDetail() {
      if (!detail || !this.text) return;
      const link = detail.relationship;
      const content = link
        ? `Relationship\n${link.source}\n${link.target}\n${link.name || "Unnamed connection"}\n${link.kind} · ${link.meaningLabel}\n${link.directionLabel}\nOriginal values: Relation ${link.relation}; Meaning ${link.meaning}; Direction ${link.direction}\nLink ID: ${link.id}\nSource ID: ${link.sourceId}\nTarget ID: ${link.targetId}\n\nConnection cycles through recorded links. Topic returns to notes.`
        : `${detail.title}\n\n${detail.notes || "No note supplied."}\n\nRecorded connections:\n${(detail.connections || []).join("\n")}\n\nConnection inspects each recorded link.`;
      const chunks = content.match(/[\s\S]{1,520}/g) || [content];
      this.page %= chunks.length;
      this.text.setAttribute("text", "value", `${chunks[this.page]}\n\nPage ${this.page + 1}/${chunks.length}`);
    },
    tick() {
      if (!this.card) this.makeCard();
      if (detail !== this.lastDetail) {
        this.lastDetail = detail;
        this.page = 0;
        this.updateDetail();
      }
      const element = document.getElementById("citation-graph");
      const renderer = element?.components?.["af-force-graph"];
      if (renderer?.nodeMesh && this.el.camera && this.labelItems.length) {
        const occupied = [];
        const panel = document.getElementById("history-panel")?.getBoundingClientRect();
        for (const { button, index } of this.labelItems) {
          const point = new THREE.Vector3().fromArray(renderer.positions, index * 3);
          renderer.nodeMesh.updateWorldMatrix(true, false);
          point.applyMatrix4(renderer.nodeMesh.matrixWorld).project(this.el.camera);
          const x = (point.x + 1) * window.innerWidth / 2;
          const y = (1 - point.y) * window.innerHeight / 2;
          const width = Math.min(180, Math.max(65, button.textContent.length * 5.8));
          let placed = false;
          for (const [dx, dy] of [[12, -12], [12, 14], [-width - 12, -12], [-width - 12, 14], [12, -38], [12, 38]]) {
            const box = { left: x + dx, right: x + dx + width, top: y + dy, bottom: y + dy + 28 };
            const overlaps = b => box.left < b.right && box.right > b.left && box.top < b.bottom && box.bottom > b.top;
            if (point.z < -1 || point.z > 1 || box.left < 8 || box.right > window.innerWidth - 8 || box.top < 8 || box.bottom > window.innerHeight - 75 || (panel && overlaps(panel)) || occupied.some(overlaps)) continue;
            button.style.left = `${box.left}px`; button.style.top = `${box.top}px`; button.style.width = `${width}px`;
            occupied.push(box); placed = true; break;
          }
          button.hidden = !placed;
        }
      }
      if (!renderer || !graph || this.revision === revision) return;
      this.revision = revision;
      const layoutSignature = JSON.stringify({ nodes: graph.nodes, links: graph.links });
      if (!renderer.nodeMesh || this.layoutSignature !== layoutSignature) {
        renderer.buildGraph(graph, { preserveView: Boolean(renderer.nodeMesh) });
        this.layoutSignature = layoutSignature;
      } else {
        // Selection and path highlights only change colors, not the layout.
        renderer.clearSelection();
        renderer.focusAnimation = null;
        graph.nodes.forEach((node, index) => {
          renderer.nodeMesh.setColorAt(index, renderer.topicColor(node.topic || "Unclassified"));
        });
      }
      this.labels.replaceChildren();
      this.labelItems = graph.nodes.map((node, index) => ({ node, index }))
        .sort((a, b) => Number(b.node.id === graph.selected) - Number(a.node.id === graph.selected)).slice(0, 30)
        .map(({ node, index }) => {
          const button = document.createElement("button");
          button.textContent = node.title;
          button.title = node.title;
          button.addEventListener("click", () => notify("citation-node-selected", node.id));
          this.labels.appendChild(button);
          return { button, index };
        });
      const pathIds = new Set(graph.path || []);
      graph.nodes.forEach((node, index) => {
        if (pathIds.has(node.id)) renderer.nodeMesh.setColorAt(index, new THREE.Color("#ba7606"));
        if (node.id === graph.selected) {
          renderer.selectedIndex = index;
          renderer.lastSelectedIndex = index;
          renderer.lastSelectedId = node.id;
          renderer.nodeMesh.setColorAt(index, new THREE.Color("#16354b"));
        }
      });
      if (renderer.nodeMesh.instanceColor) renderer.nodeMesh.instanceColor.needsUpdate = true;
      // Highlight path segments while preserving all relationship identities.
      const colors = new Float32Array(graph.links.length * 6);
      graph.links.forEach((edge, index) => {
        const onPath = (graph.path || []).some((id, i, list) => i + 1 < list.length &&
          ((id === edge.source && list[i + 1] === edge.target) || (id === edge.target && list[i + 1] === edge.source)));
        const color = new THREE.Color(onPath ? "#d39a27" : "#608092");
        color.toArray(colors, index * 6); color.toArray(colors, index * 6 + 3);
      });
      renderer.linkLines.geometry.setAttribute("color", new THREE.BufferAttribute(colors, 3));
      renderer.linkLines.material.vertexColors = true;
      renderer.linkLines.material.color.set("#ffffff");
      renderer.linkLines.material.opacity = 0.45;
      renderer.linkLines.material.needsUpdate = true;
      if (restoreCamera) {
        const saved = restoreCamera;
        restoreCamera = null;
        requestAnimationFrame(() => requestAnimationFrame(() => {
          const apply = (target, value) => {
            if (!target || !value) return;
            for (const [field, count] of [["position", 3], ["quaternion", 4], ["scale", 3]]) {
              if (Array.isArray(value[field]) && value[field].length === count && value[field].every(Number.isFinite)) target.object3D[field].fromArray(value[field]);
            }
          };
          apply(element, saved.graph);
          apply(document.getElementById("rig"), saved.rig);
          apply(document.getElementById("research-camera"), saved.camera);
        }));
      }
    },
    remove() {
      this.el.removeEventListener("enter-vr", this.onEnter);
      this.el.removeEventListener("exit-vr", this.onExit);
      this.card?.remove();
      this.labels?.remove();
    },
  });
})();
