# Release verification

Verified on 14 October 2026 using Linux x86-64 containers through Docker Desktop
in Ubuntu WSL. Docker Engine version: 29.8.1.

| Check | Result |
| --- | --- |
| Both standalone Compose configurations | Valid |
| Caddy configuration | Valid |
| Backup script POSIX shell syntax | Valid |
| `cargo test --locked` | 12 tests passed |
| Full Docker frontend and gateway release build | Passed |
| Fresh database startup and seed job | Passed; seed exited 0 |
| Frontend HTML, WASM, and JS asset serving | Passed; WASM uses `application/wasm` |
| Citation API | 50 nodes, 253 links |
| History summary API | 262 topics, 418 links, 58 attachments |
| Included database backup script | Passed; services restarted |
| Restore into a separate empty volume | Passed; both collections matched the originals |
| Seed after restore | Preserved both existing graphs |
| Release archive and SHA-256 checksum | Verified |
| Bundled browser libraries, fonts, webcam WASM/model | Checksums verified; no CDN required |
| Browser exploration and webcam inference with external requests blocked | Passed |
| Gateway/database with outbound network disabled | Passed |
| Build using only an extracted deployment ZIP | Passed |
| Leap companion launch/reconnect browser flow | Passed using a simulated native connection |
| Windows companion installer and background bridge builds | Passed; both use GUI subsystem, no console |
| Native bridge background startup and Hyperion connection | Passed on the developer's Windows machine |
| Native duplicate-launch handling and unapproved-origin rejection | Passed |
| Driver-loading failure recovery | Passed; tracking resumed in the same process after the driver was corrected |
| Companion restart of a failed bridge | Passed with corrected settings; healthy running bridge retained |
| App restart after an open Leap connection reports an error | Passed; old socket closed, companion launched once, reconnect succeeded |
| Backup concurrency/failure regression tests | 3 passed, including cross-checkout overlap and lock retention through restart |
| Real Docker backup with the default per-volume lock | Passed; archive created, services restarted, history counts preserved |

The tested Oxigraph image is pinned in `.env.cloud.example`:

```text
ghcr.io/oxigraph/oxigraph@sha256:7532c1f9aa5f28c0dc6a4243198f7f6144017fab7429bf6c164a89fec9411767
```

The checks used isolated Compose projects. Their temporary containers and
database volumes were removed after testing; the developer's database was
untouched.

## Server-only release

The `meshgraph-vps-server.zip` variant was additionally checked on 14 October
2026. It omits the Windows companion installer/manifest, C# launcher source,
native bridge source, and PowerShell build script. The native checks in the
table above apply to the complete release, not to an included installer in this
variant.

- ZIP integrity, source-file comparison, and all 14 browser asset hashes passed.
- Windows Defender custom archive scan reported no threats.
- Packaging from an extracted server-only ZIP passed.
- Docker frontend/gateway build and fresh database seeding passed using only
  the extracted server-only release.
- Three browser checks passed: both collections and webcam model with external
  requests blocked; omitted-installer explanation; app restart of an already
  installed failed companion. The included-installer check was deliberately
  skipped because this variant contains no installer.
- All 21 gesture regression tests passed.

These checks do not establish which file Google flagged or guarantee acceptance
by Google Drive or Gmail.

Public DNS/certificate issuance, integration with the professor's existing proxy,
visual browser acceptance, and physical webcam/VR/Leap Motion behavior require
the on-server and browser checks in `DEPLOYMENT.md`. This record does not claim
load testing or verification on a different CPU architecture.
