# JPL History Pilot — Evaluation Guide

## What to evaluate

Can a reader unfamiliar with the laboratory find a topic, explain a recorded
connection, consult the supplied evidence, and return to their exploration?

This is a browsing pilot. It imports the supplied archive without deduplicating
people, inventing historical dates, or inferring relationship directions. The
editorial tour follows verified links: laboratory → People → James F. Blinn →
People → laboratory → Stories → a galaxy for Man and the Cosmos.

## Reader tasks

| Task | Expected evidence | Observation |
| --- | --- | --- |
| Follow the guided introduction, leave it to explore, and resume. | Reader can identify the current topic and return to the tour. | Record confusion and navigation steps. |
| Search for “supernova” and open its topic. | The galaxy anecdote is readable and clearly attributable to the supplied archive. | Ask what is a recollection versus a verified historical claim. |
| Inspect one connection. | Reader distinguishes hierarchy/cross-link/type/tag and sees unresolved direction. | Ask them to explain it in their own words. |
| Search “Tony Longson”. | Distinct source records remain distinguishable. | Ask what further information would establish identity. |
| Set the laboratory as a path start, then find a path to the galaxy anecdote. | Path uses ordinary recorded links; it does not assert causation. | Ask what the path does and does not establish. |
| Save a named viewpoint, reload, and restore it. | Selected topic, expanded neighborhood, filters, and camera return. | Record whether the restored view is recognizable. |
| Open the archive quality report. | Missing files and uncertain direction values are apparent. | Identify the most useful next source contribution. |

Do not record an evaluation as passed until a reader actually performs it.
Capture completion, time if useful, difficulty, and the reader's explanation.

## Verification coverage

Initial implementation verification completed on 2026-09-30:

- 12 Rust unit tests passed (11 gateway/import/history tests and one Hyperion layout test).
- Five Python importer tests passed.
- All 11 Playwright history/citation tests passed against the actual gateway and ephemeral Oxigraph service.
- Rust formatting, native and WASM compilation, native and WASM lint checks, and Trunk build passed.
- Overview and source-note screenshots were inspected/recorded with a walkthrough at `demo/jpl-history/walkthrough.webm`.
- The upstream proc-macro-error2 dependency emits a Rust future-compatibility notice; current builds pass.

These results verify software behavior. Reader evaluation and physical-device
acceptance below remain open.

- Importer: deterministic conversion, malformed JSON locations, dangling links,
  path containment, HTML text extraction, and external-file handling.
- Rust: RDF parsing/roundtrip and repeat-load counts, source UUID validation,
  shortest path policy, reverse traversal metadata, and upstream error mapping.
- Browser: live history API, local notes/images, unavailable attachments, note
  search, relationship inspection, back/forward, guided introduction, bookmark
  restore, and an XR detail-card simulation.
- Existing citation tests use current controls and bounded temporal layout.
  Old assertions referencing removed HUD/select controls and fixed ±12 depth
  were updated to match the pre-pilot source. GPU batching, selection, topology,
  year filtering, controller configuration, and Hyperion frame selection remain
  covered.

## Device and deployment limits

Browser simulation verifies XR setup and card visibility; it does not establish
headset readability, comfort, tracking accuracy, or controller usability.
Physical headset, webcam, and Leap hardware sessions require separate results.

Docker Desktop was unavailable during initial implementation. Integration uses
the real Oxigraph crate in a separate ephemeral HTTP test service and the actual
Axum gateway. Compose persistence/restart behavior remains a separate acceptance
check when Docker is available.

## Questions for the professor

1. What do the observed direction values -1 and 5 mean in this export version?
2. Does the guided route organize the material in a useful and faithful way?
3. Which repeated names are distinct subjects versus duplicate records?
4. Which missing external attachments should be supplied next?
5. Which questions about the laboratory should this explorer help readers answer?

## Next decision

Use the reader's results to decide whether to improve navigation, enrich source
material, or proceed toward authoring and annotation. Completion of automated
checks does not substitute for the professor's historical review.
