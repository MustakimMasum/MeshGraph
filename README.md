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

Desktop mouse/keyboard, WebXR controllers, and webcam gestures share the same
scene:

- **Open palm drag** rotates the constellation.
- **Pinch** casts a Three.js ray and selects an instanced paper node.
- **Two-hand spread** scales the constellation.
- **Index sweep** filters visibility by publication year.
- **WebXR controllers** raycast the instanced graph and use the left thumbstick
  for locomotion.

MediaPipe `HandLandmarker` runs in a module Web Worker. Frames use transferable
`ImageBitmap` objects. Landmark coordinates use `SharedArrayBuffer` when the
browser is cross-origin isolated, and transferred `ArrayBuffer` objects as a
fallback. Raw frame-level landmarks never cross into Leptos/WASM; only macro
events such as paper selection and year-filter changes do.

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

## Verification

```sh
cargo fmt --check
cargo check
cargo check --target wasm32-unknown-unknown
cargo test
cargo clippy --all-targets --all-features -- -D warnings
trunk build
npm run test:webxr
```

Playwright tests expect a running MeshGraph stack at `http://127.0.0.1:3000`.

## Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `DATABASE_URL` | `http://localhost:7878/query` | Oxigraph SPARQL query endpoint |
| `DATABASE_UPDATE_URL` | Derived as `/update` | Oxigraph SPARQL update endpoint |
| `OPENALEX_EMAIL` | unset | Optional contact for OpenAlex requests |

Generated `dist/` and `local_graph_store/` content should not be committed or
edited manually.
