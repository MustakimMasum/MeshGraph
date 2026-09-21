# MeshGraph

MeshGraph is a spatial research-citation explorer built with Rust, WebAssembly,
Oxigraph, A-Frame, Three.js, WebXR, and MediaPipe. It ingests a DOI or arXiv
identifier through OpenAlex and turns the resulting RDF citation network into an
interactive 3D constellation.

## Architecture

MeshGraph retains three isolated layers:

1. **Knowledge graph** — Oxigraph persists `schema:ScholarlyArticle` resources,
   `cito:cites` edges, publication dates, topics, abstracts, identifiers, and
   open-access links in a named RDF graph.
2. **Gateway** — Axum validates requests, reads OpenAlex, creates RDF through
   Oxigraph RDF model types, submits SPARQL updates, and exposes typed citation,
   co-citation, bibliographic-coupling, and paper APIs.
3. **Spatial client** — Leptos CSR owns application state and semantic panels.
   A custom A-Frame component owns a Three.js `InstancedMesh` for all nodes and
   a single `LineSegments` buffer for all links.

The graph renderer does not create one DOM element per paper. Node selection is
resolved with Three.js `instanceId` ray intersections, including WebXR controller
rays.

## API

### Ingest a citation neighborhood

```http
POST /api/v1/citations/ingest
Content-Type: application/json

{"doi":"10.48550/arXiv.1706.03762","depth":2}
```

The `doi` field also accepts an arXiv ID, DOI URL, arXiv URL, or OpenAlex `W` ID.
Depth is capped at 3 and each request is capped at 250 works to protect the free
OpenAlex API and local database.

### Read topology

```http
GET /api/v1/citations/graph
GET /api/v1/citations/graph?mode=co-citation
GET /api/v1/citations/graph?mode=bibliographic-coupling
```

The response is `{ "nodes": [...], "links": [...], "mode": "..." }`. Nodes
include `id`, `title`, `year`, `topic`, and `citationCount`; weighted links expose
the selected topology.

### Read a paper

```http
GET /api/v1/citations/paper/W2741809807
```

The response contains RDF-backed metadata, abstract text, DOI, and the preferred
open-access or landing-page URL supplied by OpenAlex.

## Spatial rules

- Publication year is fixed to the local Z axis: older work is placed at negative
  Z and newer work at positive Z.
- Topics define X/Y cluster attractors. A bounded O(nodes + links) force pass
  adjusts the cluster layout without quadratic pairwise repulsion.
- Citation count controls node radius.
- Citation nodes use one `THREE.InstancedMesh`; citation edges use one dynamic
  `THREE.LineSegments` geometry.
- Citation, co-citation, and bibliographic-coupling modes can be switched without
  rebuilding the Leptos UI.

## Input modes

Desktop mouse/keyboard, WebXR controllers, webcam gestures, and an Ultraleap
camera through Hyperion share the same scene. Use the **Hand input** selector
in the top-right toolbar to choose **Webcam · MediaPipe** or
**Leap Motion · Hyperion**, then select **Toggle**.

- **Webcam:** open-palm movement pans, point and pinch selects, two-hand spread
  scales, and a horizontal swipe rotates the constellation.
- **Leap Motion / Hyperion:** index-finger movement controls the snapping
  reticle, pinch selects, an open palm pans, a fast horizontal hand movement
  rotates, and two-hand spread scales.
- **WebXR controllers** raycast the instanced graph and use the left thumbstick
  for locomotion.

MediaPipe `HandLandmarker` runs in a dedicated Web Worker. Frames use transferable
`ImageBitmap` objects. Landmark coordinates use `SharedArrayBuffer` when the
browser is cross-origin isolated, and transferred `ArrayBuffer` objects as a
fallback. Raw frame-level landmarks never cross into Leptos/WASM; only macro
events such as paper selection and year-filter changes do.

The Hyperion path uses a separate, Windows-only Rust bridge. The bridge loads
the `LeapC.dll` installed by Hyperion at runtime and publishes only the latest
tracking frame over `ws://127.0.0.1:6437/hands`. Browser-side gesture recognition
maps LeapC palm, digit, velocity, pinch, and grab values to the existing scene
macros. Sensor data never passes through the Axum graph gateway.

The gateway sends `Cross-Origin-Opener-Policy: same-origin` and
`Cross-Origin-Embedder-Policy: credentialless` so supported browsers can enable
shared memory. Camera access requires `localhost` or HTTPS.

## Run

Optionally set `OPENALEX_EMAIL` to identify requests to the OpenAlex polite pool,
then start the local stack:

```sh
docker compose up --build
```

Open `http://localhost:3000`. Oxigraph is exposed at `http://localhost:7878`.
The Compose seed job loads a small research network so the scene is useful before
the first live ingestion.

For Leap Motion input on Windows, start the host-side Hyperion bridge in a
separate terminal. It must run on the Windows host rather than inside Docker so
it can access the installed Hyperion service and `LeapC.dll`:

```powershell
cargo run --bin hyperion_bridge
```

The bridge defaults to the SDK location installed by Hyperion. Override it only
when using a custom installation:

```powershell
$env:HYPERION_LEAPC_PATH = "D:\Ultraleap\LeapSDK\lib\x64\LeapC.dll"
cargo run --bin hyperion_bridge
```

Wait for the gateway to report:

```text
MeshGraph gateway listening on http://localhost:3000
```

Stop the stack with:

```sh
docker compose down
```

For local development, install Trunk and the WASM target:

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
docker compose up -d oxigraph_db graph_seed
trunk build
cargo run
```

`trunk build` writes the CSR bundle to `dist/`; the Axum process serves that
directory and the API on port 3000.

## Manual testing

### Constellation and ingestion

After opening `http://localhost:3000`, confirm that the seeded constellation is
visible, then:

1. Click a colored paper node and confirm that its metadata panel opens.
2. Switch between **Citations**, **Co-citation**, and **Coupling**.
3. Use the mouse to look around and WASD to move through the scene.
4. Enter `arXiv:1706.03762`, choose `1 hop`, and select **Map constellation**.
5. Confirm that the paper and relationship totals update after ingestion.

Start with depth 0 or 1 when testing. Higher depths can approach the 250-work
per-request safety limit and take longer to complete.

### Webcam gestures

Choose **Webcam · MediaPipe**, select **Toggle**, and grant camera access. Test
each mapping:

- Slow open-palm movement pans the constellation.
- Pinching over a node selects that paper.
- A two-hand spread zooms the constellation.
- A quick horizontal hand or index-finger swipe rotates the constellation.

The HUD reports whether tracking is using `shared memory` or the
`transferable buffers` fallback. Use `localhost` or HTTPS: camera access and
cross-origin isolated shared memory are not generally available from an insecure
LAN address.

### Original Leap Motion Controller

The Hyperion 6.2 installer includes the tracking service, LeapC SDK, and Control
Panel needed by MeshGraph. The bridge currently targets the packed LeapC ABI
shipped with Hyperion 6.2:

1. Install Ultraleap Hyperion from the Leap Motion Controller download page.
2. Connect the original Leap Motion Controller and confirm it is visible in the
   Ultraleap Control Panel visualizer.
3. Run `cargo run --bin hyperion_bridge` from this repository and wait for the
   `ws://127.0.0.1:6437/hands` listening message.
4. Open MeshGraph in Chrome or Edge, choose **Leap Motion · Hyperion**, and
   select **Toggle**.

The HUD reports bridge, tracking-service, camera, and hand-presence state. The
bridge endpoint `/health` returns its latest status or tracking frame for
troubleshooting. Set `HYPERION_BRIDGE_ADDR` to change the loopback listener from
its default `127.0.0.1:6437`.

### WebXR

With a headset connected and its runtime running, open MeshGraph in Chrome or
Edge and use the VR button in the lower-right corner. Point either controller at
a paper and pull the trigger to select it. The left thumbstick controls
locomotion.

### API smoke tests with PowerShell

Read the direct citation graph:

```powershell
Invoke-RestMethod http://localhost:3000/api/v1/citations/graph
```

Read a derived topology:

```powershell
Invoke-RestMethod "http://localhost:3000/api/v1/citations/graph?mode=co-citation"
```

Exercise the live OpenAlex-to-RDF ingestion path:

```powershell
$body = @{
    doi = "arXiv:1706.03762"
    depth = 0
} | ConvertTo-Json

$result = Invoke-RestMethod `
    -Method Post `
    -Uri http://localhost:3000/api/v1/citations/ingest `
    -ContentType "application/json" `
    -Body $body

$result
Invoke-RestMethod "http://localhost:3000/api/v1/citations/paper/$($result.seedId)"
```

## Verification

On a fresh checkout, install the browser-test dependencies first:

```sh
npm ci
```

Keep the MeshGraph stack running while executing the Playwright suite, because
the tests connect to `http://127.0.0.1:3000`.

```sh
cargo fmt --check
cargo check
cargo check --target wasm32-unknown-unknown
cargo test
cargo clippy --all-targets --all-features -- -D warnings
trunk build
npm run test:webxr
```

The Playwright coverage checks instanced rendering, temporal Z placement, paper
selection, topology switching, year filtering, and WebXR controller setup.

If the scene is empty or a service is unavailable, inspect the stack with:

```sh
docker compose ps
docker compose logs graph_seed oxigraph_db axum_gateway
```

## Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `DATABASE_URL` | `http://localhost:7878/query` | Oxigraph SPARQL query endpoint |
| `DATABASE_UPDATE_URL` | Derived as `/update` | Oxigraph SPARQL update endpoint |
| `OPENALEX_EMAIL` | unset | Optional contact for OpenAlex requests |

Generated `dist/` and `local_graph_store/` content should not be committed or
edited manually.
