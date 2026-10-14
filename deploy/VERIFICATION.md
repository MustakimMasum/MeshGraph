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

The tested Oxigraph image is pinned in `.env.cloud.example`:

```text
ghcr.io/oxigraph/oxigraph@sha256:7532c1f9aa5f28c0dc6a4243198f7f6144017fab7429bf6c164a89fec9411767
```

The checks used isolated Compose projects. Their temporary containers and
database volumes were removed after testing; the developer's database was
untouched.

Public DNS/certificate issuance, integration with the professor's existing proxy,
visual browser acceptance, and physical webcam/VR/Leap Motion behavior require
the on-server and browser checks in `DEPLOYMENT.md`. This record does not claim
load testing or verification on a different CPU architecture.
