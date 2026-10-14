# Bundled browser dependencies

The app serves these files from its own origin, including optional webcam hand
tracking. No CDN or connection to the developer's computer is used to load them.

- A-Frame 1.4.2: MIT; license in `aframe/LICENSE`.
- Roboto MSDF font atlas supplied by A-Frame: Apache 2.0; `aframe/Roboto-LICENSE`.
- MediaPipe Tasks Vision 0.10.35: Apache 2.0; license in `mediapipe/LICENSE`.
- Google MediaPipe Hand Landmarker float16 model, version 1, downloaded from the
  official MediaPipe model distribution. Its source URL is in the manifest.

`manifest.json` records the source, size, and SHA-256 of each downloaded file.
NPM tarball integrity is checked before files are extracted. The dependency
versions match the previously used CDN versions.

Maintainers can refresh the bundled files with
`python scripts/vendor-runtime-assets.py`, then rebuild the frontend and verify
it before creating a new release. VPS administrators do not need to run this
command; the deployment ZIP already includes all of these assets.
