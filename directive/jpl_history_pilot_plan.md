# JPL History Pilot — Execution Plan

Status: Pilot implemented; automated integration verification passed. Physical-device,
Docker persistence, and professor evaluation remain explicit acceptance items.

Implementation decisions: the importer uses standard-library Python for JSONL,
HTML text extraction, SHA-256 hashing, and deterministic output. The gateway and
application state remain Rust. This replaces the initially proposed Rust import
command and avoids introducing parsing dependencies into the application. The
small archive is stored as native RDF identities/relationships plus a versioned
source-preserving snapshot, which the gateway reads from Oxigraph. See README
for commands and the evaluation guide for remaining acceptance items.

## Outcome

Build a JPL History collection in MeshGraph through which a new reader can learn about the Computer Graphics Laboratory, investigate connections, inspect supporting material, and return to a saved exploration.

Evaluate three complete journeys:

1. Learn about the laboratory through a guided route.
2. Investigate a question through search, neighborhood exploration, and connection paths.
3. Save an exploration and restore it after reopening the application.

## Verified starting point

- The supplied export contains 262 thoughts, 418 links, and 58 attachment records. All link endpoints resolve to thoughts.
- Thoughts include 247 normal records, five type definitions, nine tags, and one pinned record. Repeated names have distinct IDs.
- One attachment belongs to the brain itself and is a wallpaper. Attachment ownership cannot assume a thought in every case.
- Relationship direction values include -1 and 5; the supplied C# enum does not fully describe these values.
- The export includes HTML and Markdown notes, URLs, icons, and references to external files. Record timestamps are not historical event dates.
- The professor describes the archive as incomplete. His C# program is a checker extracted from a separate Neo4j translator.
- The current application uses Oxigraph, Axum, and Leptos/A-Frame/Three.js for citation exploration. Queries, detail panels, layout, and events contain citation-specific assumptions.
- The original rover specification and project brief describe an earlier experience. Retain their three-layer architectural boundary and update documentation to explain the current citation and history collections.

## Included experience

| Capability | Pilot behavior |
| --- | --- |
| Collection selection | Open Citations or JPL History with an explicit collection identity. |
| Initial history view | Focus on the laboratory and a bounded immediate neighborhood. |
| Neighborhood exploration | Expand neighbors, focus a selection, collapse added branches, and return home. |
| Search | Search names and readable note text; show matching excerpts and distinguish repeated names by context and ID. |
| Filters | Filter by recorded types, tags, and relationship kinds; distinguish organizational records from historical subjects. |
| Details and sources | Read available notes, inspect original metadata, open external URLs, and see attachment availability. |
| Relationship inspection | Inspect endpoints, name, original relation/meaning/direction values, and any supported interpretation. |
| Exploration history | Back/forward navigation restores selection, visible neighborhood, and filters. |
| Guided introduction | One short, curated route using verified source connections, with free exploration available throughout. |
| Connection paths | Find one shortest hop path between two topics under an explicit relationship policy; display every traversed relationship. |
| Saved viewpoints | Local browser bookmarks restore collection, selected topic, expanded nodes, filters, and camera transform. |
| Archive quality | Read-only report of unsupported values, unavailable attachments, repeated names, and import integrity. |
| Inputs | Desktop acceptance first; verify essential history interactions through existing WebXR controls and smoke-check existing gesture inputs. |

Defer authoring, collaborative annotation, TheBrain synchronization, automatic identity merging, inferred historical claims, a historical timeline, new 3D artifact models, and public deployment.

## Architecture and data decisions

Keep storage, gateway, and presentation separate. Import into a dedicated JPL named RDF graph. The browser requests history data through Axum.

Use a Rust importer that runs locally against an explicit export directory. Preserve brain and record IDs as stable resource identifiers. Retain source fields, numeric values, and import provenance even when their meaning is unresolved. Record source file hashes and import/schema versions so reports and bookmarks can identify the dataset they refer to.

Represent links as resources with their own IDs and properties, then derive navigable edges from them. This preserves parallel links, link metadata, and direction uncertainty. Preserve TypeId as well as explicit type/tag links and report inconsistencies. Keep attachment ownership capable of referencing a brain or a thought.

Separate raw source values from display interpretations. Use friendly labels only where supported by the export and supplied definitions. Unresolved direction values remain visible without invented arrows. Do not turn hierarchical membership into authorship, employment, or historical causation.

Use verified types or explicit category membership for grouping. Support multiple memberships and an unclassified state. Default node size is uniform; history layout uses spatial grouping without interpreting record dates as event dates. Organizational nodes can be revealed separately.

For notes, derive readable text for search and an escaped text/Markdown presentation. If rich HTML is retained, sanitize it before display. Resolve local attachments through an ID-based manifest rooted in the supplied export; external filesystem references are unavailable rather than served from arbitrary host paths. Icons may reside under a thought's .data directory. Brain-level attachments require a separate ownership path.

Use a local browser store for bookmarks with a versioned schema. Restore against current IDs and report missing records after a dataset change. Guided text is identified as editorial and tied to source node/link IDs.

## Execution phases

### Phase 1 — Source audit and interpretation contract

Deliver:

- A reproducible inventory and import report, including record counts, ID uniqueness, endpoint integrity, attachment resolution, encoding issues, and unsupported fields/values.
- A documented mapping of thoughts, links, types, tags, notes, and attachments to RDF and display concepts.
- A small set of actual source records used as fixtures for parsing and relationship interpretation.

Acceptance: reproduce the baseline counts; account for all 58 attachment records including the brain wallpaper; preserve duplicate names independently. Document unknown direction semantics and any questions for the professor. Unknown semantics do not block raw preservation or neutral navigation.

### Phase 2 — Repeatable import and isolated storage

Deliver:

- A local importer with explicit input/output arguments, validation-only mode, RDF output, and machine-readable report.
- A validated attachment manifest and note text index inputs.
- A documented load procedure for the dedicated JPL graph. Validate fully before replacing that collection; repeating an import must not duplicate records.

Suggested locations: src/thebrain/ for source parsing/mapping, src/bin/import_thebrain.rs for the command, and data/jpl-history/ for intentionally checked-in derived artifacts. Do not commit generated database persistence.

Acceptance: malformed records report filename and line number; imports preserve IDs and original link fields; a repeated import produces the same graph content; citation data remains queryable; a failed validation leaves the loaded collection usable.

### Phase 3 — History API and query behavior

Deliver focused handlers and typed responses for these proposed routes:

| Route | Purpose |
| --- | --- |
| GET /api/v1/history/summary | Collection identity, counts, import version, root ID, available filters. |
| GET /api/v1/history/neighborhood | Bounded graph around a requested node with explicit edge policy and truncation metadata. |
| GET /api/v1/history/nodes/:id | Node details, notes, classifications, and attachment references. |
| GET /api/v1/history/links/:id | Original and interpreted relationship properties. |
| GET /api/v1/history/search | Bounded/paginated name and note search with excerpts. |
| GET /api/v1/history/path | A shortest hop path, its traversal directions, applied policy, or a clear no-path result. |
| GET /api/v1/history/attachments/:id | Manifest-resolved local content where available. |
| GET /api/v1/history/quality | Read-only import and archive quality report. |

Path search defaults to ordinary hierarchy/cross-links and excludes system/type/tag links. Connectivity search may traverse an edge in either direction, but must show original orientation and uncertainty. Filters and traversal policy must be stated in results. No-path results must distinguish disconnected data from a configured search limit.

Acceptance: IDs and limits are validated; errors are structured; unknown IDs return clear responses; relation records are not collapsed merely because endpoints match; external file references cannot become arbitrary file reads.

### Phase 4 — Desktop vertical slice

Deliver collection selection, the laboratory starting view, node selection, neighborhood expansion/collapse, focus/home controls, readable notes, and attachment availability.

Extract reusable rendering/input behavior from public/constellation.js as needed while keeping collection-specific layout and semantics explicit. Keep Leptos responsible for selection and panel state. A relationship can be inspected from a selected node's connection list, avoiding dependence on precise line picking.

Acceptance: complete a real source-backed route from the laboratory to a note; expand and collapse without losing unrelated visible nodes; keep labels and panels usable; open the citation collection successfully. This is the first runnable milestone.

### Phase 5 — Research and orientation features

Deliver name/note search, filters, relationship inspector, exploration back/forward, and connection-path highlighting. Selecting a result outside the visible neighborhood loads and focuses its context. A hidden selection has a clear reveal action.

Acceptance: find a node through a note phrase; distinguish two records sharing a name; explain one relationship using its original fields; navigate a computed path; restore the prior exploration with Back. Connection paths must not generate historical claims beyond their source edges.

### Phase 6 — Guided route, bookmarks, and archive quality

Deliver one guided introduction, local saved viewpoints with rename/delete controls, and the quality report view. Curate the route only after verifying its actual node/link sequence; the earlier Man and the Cosmos example is a candidate, not an assumed complete path.

Acceptance: leave a tour for free exploration and resume it; save/reopen an exploration with selection, filters, neighborhood, and camera restored; gracefully restore a bookmark with unavailable IDs; inspect missing files and repeated names without changing source data.

### Phase 7 — XR, verification, and evaluation package

Deliver controller selection/focus and an in-headset way to read essential node and relationship details. Desktop panels alone do not satisfy this requirement. Exercise existing gesture input against the history graph and record hardware-dependent limitations.

Complete meaningful importer/API tests, browser journey tests, citation regression checks, and XR setup/input tests. Use a real headset for final immersive acceptance when available; record automated simulation and physical-device results separately.

Deliver a short demonstration recording, import/quality report, reproducible README instructions, and an evaluation sheet for the professor.

Acceptance: the three primary journeys pass; available notes are readable; unresolved semantics and unavailable content are apparent; evaluation records what a reader learned, where they became disoriented, and what source information is needed next.

## Delivery order and checkpoints

Execute phases 1 → 2 → 3 → 4 → 5 → 6 → 7. Each phase should be a focused, reviewable change with its acceptance evidence. Feedback can refine the next phase without requiring a new permission step for every implementation detail once implementation is authorized.

- Checkpoint A, after phase 2: trustworthy data conversion and an explicit uncertainty report.
- Checkpoint B, after phase 4: a usable desktop path from graph selection to source material.
- Checkpoint C, after phase 6: all expanded research journeys available.
- Checkpoint D, after phase 7: evaluated pilot with reproducible setup and documented device coverage.

Do not assign firm dates until the phase 1 audit confirms mapping/attachment work and the XR hardware available for acceptance. Link semantics and in-headset detail presentation are the main effort uncertainties.

## Verification policy

Run targeted tests when implementing parsing, mapping, query behavior, and state restoration. At integration checkpoints run cargo fmt --check, cargo check, cargo test, cargo check --target wasm32-unknown-unknown, cargo clippy --all-targets --all-features -- -D warnings, trunk build, and npm run test:webxr as appropriate to the affected targets. Browser tests require the documented local stack.

Follow AGENTS.md during quick visual iterations: do not run builds/tests for those iterations unless verification is explicitly requested. Complete integration verification at the planned checkpoints. Separate pre-existing failures from pilot regressions; do not weaken assertions just to obtain a pass.

## Completion criteria

- Source IDs, records, attachment ownership, and link semantics are preserved and traceable.
- All included experience capabilities have acceptance evidence or an explicitly recorded external blocker, such as unavailable headset hardware.
- Desktop journeys and citation regression checks pass.
- Missing content and unknown meanings are represented honestly.
- Setup/import commands work from documented inputs; generated outputs are identified.
- The professor can review a demonstration and specific questions about usefulness and source interpretation.
