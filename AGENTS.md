# Repository Guidelines

## Project Structure & Module Organization

The implementation specification is in `directive/system_prompt.md`. Keep code changes aligned with its three-layer architecture.

The application layout is:

- `docker-compose.yml`: local Oxigraph database and Axum gateway services.
- `Cargo.toml`: shared Rust dependencies and release settings.
- `index.html`: Leptos CSR host page with A-Frame.
- `src/main.rs`: Axum API gateway and static-file server.
- `src/app.rs`: Leptos/WASM 3D client.
- `local_graph_store/`: generated Oxigraph persistence data; do not commit it.
- `dist/`: generated frontend assets served by the gateway; do not edit by hand.

Keep database, gateway, and frontend concerns separated. Shared response types may be extracted into focused Rust modules when needed.

## Build, Test, and Development Commands

- `cargo check`: quickly validate Rust code and dependencies.
- `cargo test`: run all Rust tests.
- `trunk build`: compile the Leptos client into `dist/`.
- `cargo fmt --check`: verify formatting without modifying files.
- `cargo clippy --all-targets --all-features -- -D warnings`: enforce lint cleanliness.
- `docker compose up --build`: build and run Oxigraph plus the gateway locally.
- `docker compose down`: stop the local service stack.

Document any additional WASM build command in `README.md` and keep it reproducible.

## Coding Style & Naming Conventions

Use Rust 2021 conventions and four-space indentation. Run `cargo fmt` before committing. Use `snake_case` for modules, functions, and variables; `PascalCase` for structs, enums, and Leptos components; and `SCREAMING_SNAKE_CASE` for constants.

Keep handlers small, return structured errors, and avoid `unwrap()` in request paths. Use explicit API routes such as `/api/v1/structure` and descriptive DOM IDs such as `rover-group`.

## Testing Guidelines

Place unit tests beside the Rust code in `#[cfg(test)]` modules and integration tests under `tests/`. Name tests by behavior, for example `structure_endpoint_returns_bindings`. Cover SPARQL response parsing, environment-variable fallback behavior, and API error mapping. Run `cargo test` before opening a pull request.

## Commit & Pull Request Guidelines

There is no existing commit history to establish a local convention. Use short, imperative commit subjects, for example `Add Oxigraph compose service`. Keep commits focused.

Pull requests should explain the change, list verification commands, link relevant issues, and include screenshots or recordings for 3D viewport changes. Call out configuration changes, new ports, and generated files explicitly.
