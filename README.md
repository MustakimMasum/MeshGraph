# Cadmus

Cadmus is a local, microservice-backed 3D spatial knowledge base. Oxigraph stores
the graph, an Axum gateway exposes a narrow API, and a Leptos/A-Frame client
renders the graph as an interactive scene.

The assembled rover uses the public-domain
[Mars Sojourner Rover](https://www.printables.com/model/411486-mars-sojourner-rover)
model reuploaded by Books from the original Blend Swap model by argonius.

## Run with Docker

```sh
docker compose up --build
```

Open `http://localhost:3000`. Oxigraph is available on
`http://localhost:7878`; the one-shot `graph_seed` service loads the included
Sojourner rover graph on startup. Check gateway availability at
`http://localhost:3000/api/v1/health`.

## Local Development

Install Rust, the `wasm32-unknown-unknown` target, and Trunk:

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
```

Run the frontend and gateway in separate terminals:

```sh
trunk watch
cargo run
```

Then open `http://localhost:3000`. Axum serves each rebuilt frontend bundle from
`dist/` and keeps API requests on the same origin.

Use `cargo test`, `cargo fmt --check`, and
`cargo clippy --all-targets --all-features -- -D warnings` before committing.

Run `npm run test:webxr` to validate WebXR capability detection, session-state
recovery, controller trigger interaction, and rover explode/assemble behavior.
