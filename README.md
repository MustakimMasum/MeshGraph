# GraphMesh

GraphMesh is an interactive 3D knowledge graph prototype that demonstrates how
semantic data can provide context, relationships, and navigable structure for a
complex physical object.

Using NASA's Sojourner rover as its subject, GraphMesh lets users explode the
rover into selectable components, inspect graph-backed metadata, follow
relationships between connected parts, and review the SPARQL queries driving the
experience.

**Developer:** [Mustakim Fire Masum](https://github.com/MustakimMasum)

## Experience Highlights

- Interactive assembled and exploded rover views
- Selectable 3D components with semantic highlighting
- Graph-backed component names, categories, purposes, power requirements, and
  mission notes
- Visual relationship highlighting and connection tethers
- Oxigraph trace panel showing SPARQL queries, node hierarchy, and relationships
- Desktop mouse, keyboard, zoom, and pan controls
- WebXR capability detection, controller raycasting, and in-headset assembly
- Graceful fallback to bundled demonstration coordinates if Oxigraph is
  temporarily unavailable

## Architecture

GraphMesh separates storage, application logic, and presentation into three
layers.

### Knowledge Graph Layer

Oxigraph stores the rover graph as RDF and answers SPARQL queries. The included
Turtle dataset models rover components, spatial offsets, metadata, and physical
`connectedTo` relationships.

A one-shot Docker Compose service waits for Oxigraph to become available and
automatically seeds the included graph.

### Rust Gateway Layer

An Axum gateway provides a narrow HTTP API between the browser and Oxigraph. It
constructs SPARQL queries, validates component identifiers, parses SPARQL JSON
bindings into typed Rust structures, maps upstream failures to structured
errors, and serves the compiled frontend.

Primary endpoints:

- `GET /api/v1/health`
- `GET /api/v1/structure`
- `GET /api/v1/components/:component_name`
- `GET /api/v1/components/:component_name/related`

### Spatial Presentation Layer

The frontend is a Leptos client-side application compiled from Rust to
WebAssembly. A-Frame and Three.js render the rover and provide desktop and WebXR
interaction.

Custom A-Frame components handle semantic highlighting, relationship tethers,
camera controls, WebXR session state, and VR-only controls.

## Technology Stack

| Area | Technology |
| --- | --- |
| Knowledge graph | Oxigraph, RDF/Turtle, SPARQL |
| Backend | Rust, Axum, Tokio |
| HTTP and serialization | Reqwest, Serde, Serde JSON |
| Frontend | Rust, Leptos CSR, WebAssembly |
| 3D and XR | A-Frame, Three.js, WebXR |
| Browser integration | `web-sys`, `gloo-net` |
| Asset pipeline | STL, GLB, Node.js scripts |
| Infrastructure | Docker, Docker Compose |
| Build tooling | Cargo, Trunk |
| Verification | Rust tests, Playwright |

## Interaction

Click the assembled rover to explode it. In the exploded view:

- Click a component to highlight it and open its semantic metadata.
- Review the Oxigraph trace beneath the semantic card.
- Close the semantic card explicitly with its close button.
- Use **Assemble Rover** at the bottom center to collapse the model.

Clicking the rover or empty scene does not assemble the rover or clear the
current selection. In VR, use the in-headset assemble control.

## Run with Docker

```sh
docker compose up --build
```

Use `docker compose up -d --build` to run the stack in the background.

Open `http://localhost:3000`. Oxigraph is available at
`http://localhost:7878`, and the graph seed service loads the included Sojourner
rover dataset automatically.

## Local Development

Install Rust, the WebAssembly target, and Trunk:

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
```

Start Oxigraph and seed the graph:

```sh
docker compose up -d oxigraph_db graph_seed
```

Run the frontend and gateway in separate terminals:

```sh
trunk watch
cargo run
```

Then open `http://localhost:3000`.

## Verification

```sh
cargo fmt --check
cargo check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
npm run test:webxr
```

## Potential Applications

The GraphMesh architecture can support:

- Museum and cultural heritage object exploration
- Engineering assembly and maintenance documentation
- Industrial equipment digital twins
- Training simulations and spatial learning
- Scientific instrument visualization
- Semantic product catalogs

The rover is a focused demonstration subject. The underlying approach is
designed to generalize to other graph-modeled physical systems.

## Current Scope

GraphMesh is a working prototype intended to demonstrate architecture,
interaction, and knowledge graph integration. A production evolution could add
authentication, graph authoring workflows, larger datasets, observability,
continuous deployment, and public HTTPS hosting.

The assembled rover uses the public-domain
[Mars Sojourner Rover](https://www.printables.com/model/411486-mars-sojourner-rover)
model reuploaded by Books from the original Blend Swap model by argonius.

