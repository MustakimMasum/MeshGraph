# MeshGraph

A 3D graph explorer built with Rust, Leptos/WASM, Axum, Oxigraph, and A-Frame/Three.js.

- **Citations:** explore research papers imported through OpenAlex.
- **JPL History:** browse the professor's TheBrain archive: 262 topics, 418 connections, and 58 attachment records.

## Local development

Prerequisites: Rust, Docker with its engine running, and Node.js for browser tests.
Run commands from the repository root.

### One-time setup

```powershell
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
npm ci
```

### Start the database and app

Use this workflow for development: Docker runs Oxigraph, while Rust and Trunk
run locally. Reserve the full Docker app build for packaged releases.

```powershell
docker compose stop axum_gateway
docker compose up -d oxigraph_db graph_seed
$env:DATABASE_URL='http://127.0.0.1:7878/query'
$env:DATABASE_UPDATE_URL='http://127.0.0.1:7878/update'
cargo run --bin meshgraph_gateway
```

Open:

- [Citation explorer](http://localhost:3000)
- [JPL History](http://localhost:3000/?collection=history)
- [Oxigraph](http://localhost:7878)

Axum serves the API and the frontend bundle from `dist/` on port **3000**.
The seed job loads missing bundled collections and keeps existing named graphs.

### Work on the app

Keep the gateway running and use a second terminal:

```powershell
trunk watch
```

Trunk makes an initial build, then rebuilds when frontend sources change.
`Trunk.toml` watches source/assets and Cargo files, keeping generated reports,
recordings, database files, and docs outside the watch scope. Refresh the browser
after frontend changes. Restart `cargo run --bin meshgraph_gateway`
after backend changes. Stop these processes with **Ctrl+C**; stop the database with
`docker compose down`.

If port 3000 is occupied by the Compose gateway, run `docker compose stop axum_gateway`
before starting the local Rust gateway. If Trunk rejects `NO_COLOR=1`, set
`$env:NO_COLOR='true'`.

### Run everything in Docker

```powershell
docker compose up -d --build
docker compose ps
# Stop the stack:
docker compose down
```

The database persists in `local_graph_store/`, mounted as `/data` in Oxigraph.
Both published ports bind to `127.0.0.1`. The gateway waits for the seed job to
finish and exposes a container health check. Stop the local `cargo run` gateway
before starting the Docker gateway so port 3000 is available. Docker builds both
the Leptos frontend and Rust gateway; host-side `trunk build` is not required.

Verify recovery and collection contents without replacing graphs:

```powershell
python scripts/verify-docker-persistence.py --restart --check-seed
```

This briefly restarts and stops/starts Oxigraph, compares both collections'
counts and content hashes, then confirms a normal seed run preserves them.
The report is written to `demo/jpl-history/docker-persistence.json`. Existing
named graphs are kept even when bundled seed files change; use the explicit
import command below to update JPL alone.

### Run without Docker (temporary demo/test database)

```powershell
# Terminal 1
cargo run --example pilot_graph_server

# Terminal 2
trunk build
cargo run --bin meshgraph_gateway
```

This uses real Oxigraph in memory on port 7878. Data disappears on exit, and live
citation ingestion is unavailable. Use Docker for persistent storage and ingestion.

If an existing database occupies port 7878, isolate the demo without changing it:

```powershell
# Database terminal
$env:PILOT_GRAPH_ADDR='127.0.0.1:7879'
cargo run --example pilot_graph_server
# Gateway terminal
$env:DATABASE_URL='http://127.0.0.1:7879/query'
cargo run --bin meshgraph_gateway
```

## Controls

Use mouse drag to look around, **WASD** to move, **E/C** to move vertically, and
scroll to zoom. Select a node to read its details. The bottom-right icon buttons
provide refocus, webcam input, Leap Motion input, and VR entry. Citation topology
and depth options are in **Settings**.
The **Home** icon immediately after Reset reloads the current collection's initial
view, restoring its camera, filters, and temporary exploration state. Saved
viewpoints and database content are retained.

For Leap Motion, install Ultraleap Hyperion and start the Windows host bridge:

```powershell
cargo run --bin hyperion_bridge
```

IR controls use distinct poses:

- **One open palm:** move the selection pointer to focus a node.
- **Pinch and drag:** hold a pinch and move left/right or up/down to pan in the
  camera's view. Release the pinch to stop. Pinching a focused node selects it immediately;
  holding and moving the pinch pans while keeping that selection active.
- **One closed hand:** move horizontally or vertically to orbit smoothly around
  the selected node (or the graph center).
- **Two open palms:** spread apart to zoom in; bring together to zoom out.
  Relax/curl either hand to release zoom. Remove the second hand to resume
  single-hand navigation; each pose change starts from your current hand position.
- A pinch that releases zoom or orbit does not select or drag until you release
  and pinch again.

Movement is filtered to reduce jitter, and stops when you release the pose or
tracking is lost. Gesture regression tests run with `npm run test:gestures`.

The bridge uses `ws://127.0.0.1:6437/hands`. Camera access requires localhost or
HTTPS. XR and gesture accuracy require verification with physical devices.

## JPL History

Search names and notes, expand neighborhoods, inspect relationships, follow the
guided introduction, find connection paths, and save viewpoints locally.
The archive is incomplete; unknown directions and unavailable attachments are
shown explicitly. Record timestamps are not historical event dates.

To regenerate the bundled history, use Python 3.10+ (no additional packages):

```powershell
python scripts/import-thebrain.py "resources/JPL CGL history/JPL CGL history" --validate-only
python scripts/import-thebrain.py "resources/JPL CGL history/JPL CGL history"
```

To load only the regenerated history into a running Oxigraph instance:

```powershell
Invoke-WebRequest -Method Put -ContentType 'text/turtle' `
  -InFile 'data/jpl-history/history.ttl' `
  -Uri 'http://localhost:7878/store?graph=http%3A%2F%2Fexample.org%2Fmeshgraph%2Fgraphs%2Fjpl-history'
```

Rebuild frontend assets after importing images. `docker compose run --rm graph_seed`
loads missing graphs and preserves existing collections. To deliberately replace
**both** collections with bundled data, including removing live-ingested citation
additions, run `docker compose run --rm -e FORCE_SEED=true graph_seed`.
For a Docker gateway, run `docker compose up -d --build` after changing image assets.
Regenerate the citation seed with `npm run seed:generate`.

See the [pilot plan](directive/jpl_history_pilot_plan.md) and
[evaluation guide](directive/jpl_history_pilot_evaluation.md). Use the
[acceptance record](directive/jpl_history_acceptance_record.md) for reader, physical
input, and Docker persistence sessions. In VR, **Connection** cycles recorded
relationships with source values, **Topic** returns to notes, and **Next page**
advances long text. Record a walkthrough
with `node scripts/record-history-demo.mjs`; output goes to `demo/jpl-history/`.
An optional output directory argument writes elsewhere. The XR segment is a
browser simulation. Stop `trunk watch` before recording or testing: generated
artifacts can trigger rebuilds and interrupt page loads. Alternatively, use an
output directory outside the repository, for example
`npx playwright test --output "$env:TEMP/meshgraph-webxr"`.

## Verification

Keep the app and database running for browser tests.

```powershell
cargo fmt --check
cargo check
cargo check --target wasm32-unknown-unknown
cargo test
cargo clippy --all-targets --all-features -- -D warnings
trunk build
python -m unittest discover -s tests -p test_thebrain_import.py
npm run test:webxr
```

For an empty scene or service error:

```powershell
docker compose ps
docker compose logs graph_seed oxigraph_db axum_gateway
```

## Architecture and API

Oxigraph stores separate RDF collections. Axum queries the database and returns
typed responses. Leptos manages UI state; Three.js renders batched nodes and edges.
The history pilot preserves source fields in a versioned snapshot stored alongside
RDF identities and relationships.

| API | Purpose |
| --- | --- |
| `POST /api/v1/citations/ingest` | Import `{"doi":"arXiv:1706.03762","depth":1}`; maximum depth 3, maximum 250 fetched works. |
| `GET /api/v1/citations/graph` | Graph; optional `mode=co-citation` or `mode=bibliographic-coupling`. |
| `GET /api/v1/citations/paper/:id` | Paper metadata. |
| `GET /api/v1/history/summary`, `/graph`, `/quality` | Collection, topology, and audit report. |
| `GET /api/v1/history/neighborhood?id=…`, `/nodes/:id`, `/links/:id` | Explore history records. |
| `GET /api/v1/history/search?q=…`, `/path?from=…&to=…`, `/attachments/:id` | Search, connection paths, and supplied content. |

All abbreviated history paths in the table use the `/api/v1/history` prefix.

## Configuration

| Variable | Default / purpose |
| --- | --- |
| `DATABASE_URL` | `http://localhost:7878/query` |
| `DATABASE_UPDATE_URL` | Derived from the query URL as `/update`. |
| `PILOT_GRAPH_ADDR` | `127.0.0.1:7878`; bind address for the ephemeral demo database only. |
| `FORCE_SEED` | `false`; seed job keeps existing graphs. `true` explicitly replaces both bundled collections. |
| `OPENALEX_EMAIL` | Optional contact for OpenAlex requests. |
| `HYPERION_LEAPC_PATH` | Override the installed Hyperion `LeapC.dll` path. |
| `HYPERION_BRIDGE_ADDR` | `127.0.0.1:6437` |

Generated `dist/`, `local_graph_store/`, and `demo/` are ignored. The original
TheBrain export under `resources/` is also ignored; derived history RDF, audit
report, and copied image assets are checked in.
