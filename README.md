# Cadmus

Cadmus is a local, microservice-backed 3D spatial knowledge base. Oxigraph stores
the graph, an Axum gateway exposes a narrow API, and a Leptos/A-Frame client
renders the graph as an interactive scene.

The assembled rover uses the public-domain
[Mars Sojourner Rover](https://www.printables.com/model/411486-mars-sojourner-rover)
model reuploaded by Books from the original Blend Swap model by argonius.

## Interaction

Click the assembled rover to explode it. In the exploded view:

- Click an individual component to highlight it and open its semantic metadata.
- Review the Oxigraph trace beneath the semantic card to see the active SPARQL
  query, graph hierarchy, and relationships.
- Close the semantic card explicitly with its close button.
- Use **Assemble Rover** at the bottom center to collapse the model.

Clicking the rover or empty scene does not assemble the rover or clear the
current semantic selection. In VR, use the in-headset assemble control.

## Run with Docker

```sh
docker compose up --build
```

Use `docker compose up -d --build` to run the stack in the background.

Open `http://localhost:3000`. Oxigraph is available on
`http://localhost:7878`; the one-shot `graph_seed` service loads the included
Sojourner rover graph on startup. Check gateway availability at
`http://localhost:3000/api/v1/health`.

The gateway exposes rover structure at `/api/v1/structure` and semantic
component metadata at `/api/v1/components/:component_name`, for example
`/api/v1/components/Drill`. Connected graph nodes are available at
`/api/v1/components/:component_name/related`.

## Local Development

Install Rust, the `wasm32-unknown-unknown` target, and Trunk:

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

Then open `http://localhost:3000`. Axum serves each rebuilt frontend bundle from
`dist/` and keeps API requests on the same origin.

For quick visual iterations, update the UI and inspect it in the running app
without running the full verification suite after every small change.

## Verification

Use `cargo test`, `cargo fmt --check`, and
`cargo clippy --all-targets --all-features -- -D warnings` before committing.

Run `npm run test:webxr` to validate WebXR capability detection, session-state
recovery, controller trigger interaction, and rover explode/assemble behavior.
