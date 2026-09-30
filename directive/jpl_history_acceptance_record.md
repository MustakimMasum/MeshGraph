# JPL History — Acceptance Record

Copy this file for each session. Leave unperformed checks as **not run**. Browser
simulation does not count as a physical-device or reader evaluation pass.

## Session

- Date, evaluator, and reader:
- Commit (`git rev-parse HEAD`):
- Dataset version (`GET /api/v1/history/summary`):
- Browser/version, OS, and viewport:
- Headset/controllers, webcam, or Hyperion version (if used):
- Evidence location (recording, screenshots, logs):

## Reader journeys

Use the tasks in [the evaluation guide](jpl_history_pilot_evaluation.md). Record
the reader's own explanation before providing help. Results: pass, fail, or not run.

| Journey | Result | Time / help needed | Reader explanation and disorientation |
| --- | --- | --- | --- |
| Guided introduction; leave to explore and resume | Not run | | |
| Search “supernova”; read the supplied note | Not run | | |
| Inspect a connection and explain direction uncertainty | Not run | | |
| Distinguish the “Tony Longson” records | Not run | | |
| Find laboratory → galaxy path without inferring causation | Not run | | |
| Save, reload, and restore selection, filters, neighborhood, camera | Not run | | |
| Inspect missing files and repeated names in archive quality | Not run | | |

What did the reader learn? What evidence is missing? What should change next?

## Physical input acceptance

Start at `http://localhost:3000/?collection=history`. For headset testing, use
the supported local/secure browser setup for that device and record its origin.

| Check | Result | Device and evidence / limitation |
| --- | --- | --- |
| Left and right controller rays select a topic | Not run | |
| Focus and Expand work in the headset; Home restores laboratory | Not run | |
| Notes are readable; Next page advances and wraps | Not run | |
| Connection cycles recorded links, including beyond the first six | Not run | |
| Relationship endpoints, ID, original Relation/Meaning/Direction and uncertainty are readable | Not run | |
| Topic returns to notes; exit VR restores desktop panels | Not run | |
| Webcam pan, pinch selection, zoom, rotation | Not run | |
| Hyperion pan, pinch selection, zoom, rotation | Not run | |
| Comfort, text size, tracking reliability, accidental selections | Not run | |

## Docker persistence acceptance

Use the persistent Compose stack. Do not run the seed job again between the
before/after observations: reseeding would mask persistence failures.

1. Run `docker compose up -d oxigraph_db graph_seed` and start the gateway using
   the README instructions. Record history summary version/counts and a source
   node ID/note; confirm `/api/v1/citations/graph` still returns citation data.
2. Run `docker compose restart oxigraph_db`. Wait for it to respond, reload the
   page, and compare version/counts and the source note through the gateway.
3. Run `docker compose stop oxigraph_db`, then `docker compose start oxigraph_db`.
   Repeat the observations without reseeding. Record the commands and responses.
4. Save/reload a browser viewpoint and confirm it restores against these IDs.

| Observation | Before | After restart | After stop/start | Result |
| --- | --- | --- | --- | --- |
| History dataset version and counts | | | | Not run |
| Recorded node ID and note | | | | Not run |
| Citation graph remains queryable | | | | Not run |
| Saved viewpoint restore | | | | Not run |

## Historical review and next decision

- Professor's interpretation of direction values -1 and 5:
- Guided route corrections:
- Repeated-name identity evidence:
- Missing attachments requested:
- Priority research question:
- Decision: navigation improvements / source enrichment / proposal for authoring.
- Owner and next action:

Authoring, annotation, and public deployment remain outside this pilot until
the evaluation provides a reason to expand scope.
