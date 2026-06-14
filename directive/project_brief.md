# GraphMesh: Spatial Knowledge Graph Explorer

## Project Summary

GraphMesh is an interactive 3D knowledge graph prototype that demonstrates how
semantic data can provide context, relationships, and navigable structure for a
complex physical object.

The experience uses NASA's Sojourner rover as its subject. A user can explode
the rover into selectable components, inspect graph-backed metadata, follow
relationships between connected parts, and review the SPARQL query and result
hierarchy driving the interface.

GraphMesh was designed as a practical demonstration of combining a knowledge graph,
a typed Rust API, WebAssembly, browser-based 3D rendering, and WebXR within a
locally deployable microservice architecture.

## Experience Highlights

- Interactive assembled and exploded rover views
- Selectable 3D components with semantic highlighting
- Graph-backed component names, categories, purposes, power requirements, and
  mission notes
- Visual relationship highlighting and connection tethers between related parts
- An Oxigraph trace panel showing the active SPARQL query, node hierarchy, and
  relationships
- Desktop navigation with mouse, keyboard, zoom, and pan controls
- WebXR capability detection, headset entry, controller raycasting, and an
  in-headset assembly control
- Graceful fallback to bundled demonstration coordinates if Oxigraph is
  temporarily unavailable

## Architecture

GraphMesh separates storage, application logic, and presentation into three layers.

### 1. Knowledge Graph Layer

Oxigraph stores the rover graph as RDF and answers SPARQL queries. The included
Turtle dataset models:

- `SojournerRover hasPart Component`
- Component position offsets
- Human-readable component metadata
- Physical `connectedTo` relationships

A one-shot Docker Compose seed service waits for Oxigraph to become available,
then loads the graph automatically.

### 2. Rust Gateway Layer

An Axum service provides a narrow HTTP API between the browser and Oxigraph. It:

- Issues SPARQL queries through `reqwest`
- Parses SPARQL JSON bindings into typed Rust structures
- Validates dynamic component identifiers
- Maps upstream failures to structured HTTP errors
- Serves the compiled frontend assets

Primary endpoints:

- `GET /api/v1/health`
- `GET /api/v1/structure`
- `GET /api/v1/components/:component_name`
- `GET /api/v1/components/:component_name/related`

### 3. Spatial Presentation Layer

The frontend is a Leptos client-side application compiled from Rust to
WebAssembly. A-Frame and Three.js render the rover and provide desktop and WebXR
interaction.

Leptos signals coordinate application state, including explosion state,
selection, metadata, and related graph nodes. Custom A-Frame components handle
semantic material highlighting, relationship tethers, camera controls, WebXR
session state, and VR-only controls.

## Technology Stack

| Area | Technology | Role |
| --- | --- | --- |
| Knowledge graph | Oxigraph | RDF storage and SPARQL query execution |
| Graph data | RDF/Turtle | Rover nodes, metadata, offsets, and relationships |
| Backend | Rust, Axum, Tokio | Typed asynchronous API gateway |
| HTTP and serialization | Reqwest, Serde, Serde JSON | Oxigraph requests and SPARQL response parsing |
| Frontend | Rust, Leptos CSR, WebAssembly | Reactive browser application |
| 3D and XR | A-Frame, Three.js, WebXR | Scene rendering and immersive interaction |
| Browser integration | `web-sys`, `gloo-net` | DOM access and frontend API requests |
| Asset pipeline | STL, GLB, Node.js scripts | Rover model preparation and browser delivery |
| Infrastructure | Docker, Docker Compose | Reproducible local multi-service deployment |
| Build tooling | Cargo, Trunk | Rust and WebAssembly builds |
| Verification | Rust tests, Playwright | Gateway parsing and browser/WebXR behavior |

## Technical Decisions

### Graph-Driven Context

The 3D model is treated as an interface into a semantic graph rather than a
standalone visual asset. Selecting a component triggers metadata and relationship
queries, making the underlying graph visible and understandable to the user.

### Narrow API Boundary

The browser does not query Oxigraph directly. The Axum gateway owns query
construction, validation, error handling, and response shaping. This keeps the
frontend focused on interaction while preserving a clear service boundary.

### Shared Rust Across Backend and Frontend

Rust powers both the gateway and the Leptos WebAssembly client. This provides a
consistent type system across service and UI development while still allowing
direct integration with browser-native A-Frame and WebXR APIs.

### Reproducible Demonstration Environment

Docker Compose starts Oxigraph, seeds the graph, builds the Rust/WebAssembly
application, and runs the gateway. The complete experience can be launched with:

```sh
docker compose up --build
```

## Engineering Competencies Demonstrated

- Knowledge graph modeling with RDF and SPARQL
- Full-stack Rust application development
- Typed API design and upstream service integration
- WebAssembly-based reactive frontend development
- Interactive 3D scene design and spatial UI
- WebXR capability handling and controller interaction
- Multi-container application orchestration
- 3D asset conversion and coordinate-system troubleshooting
- Progressive fallback and structured error handling
- Automated backend and browser interaction testing

## Potential Applications

The same architecture can support:

- Museum and cultural heritage object exploration
- Engineering assembly and maintenance documentation
- Digital twins for industrial equipment
- Training simulations and spatial learning
- Scientific instrument visualization
- Semantic product catalogs

The rover is a focused demonstration subject; the underlying approach is
designed to generalize to other graph-modeled physical systems.

## Current Scope

GraphMesh is a working prototype intended to demonstrate architecture, interaction,
and knowledge graph integration. It currently uses a curated Sojourner rover
dataset and locally hosted services. A production evolution could add
authentication, graph authoring workflows, larger datasets, observability,
continuous deployment, and public HTTPS hosting.
