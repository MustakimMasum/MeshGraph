"""Create a source deployment archive without generated or private files."""

import argparse
import hashlib
import json
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--server-only", action="store_true",
                        help="omit the optional Windows Leap companion and its build sources")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    if not args.server_only:
        if not (root / "public/downloads/MeshGraphLeapSetup.exe").is_file():
            raise FileNotFoundError("Build the Windows companion with scripts/build-leap-companion.ps1 before packaging")
        companion = json.loads((root / "public/downloads/manifest.json").read_text(encoding="utf-8"))
        if hashlib.sha256((root / "public/downloads/MeshGraphLeapSetup.exe").read_bytes()).hexdigest() != companion["installer_sha256"]:
            raise ValueError("Windows companion checksum does not match its build manifest")
    manifest = json.loads((root / "public/vendor/manifest.json").read_text())
    for asset in manifest["assets"]:
        path = (root / asset["path"]).resolve()
        if not path.is_relative_to(root / "public/vendor"):
            raise ValueError("Vendor manifest path must remain inside public/vendor")
        if hashlib.sha256(path.read_bytes()).hexdigest() != asset["sha256"]:
            raise ValueError(f"Bundled runtime asset is missing or changed: {asset['path']}")
    entries = [
        "Cargo.toml", "Cargo.lock", "Dockerfile", "index.html", "Trunk.toml",
        ".dockerignore", ".gitattributes", ".env.cloud.example",
        "docker-compose.cloud.yml", "docker-compose.vps.yml", "DEPLOYMENT.md",
        "README.md", "LICENSE", "src", "public", "data", "deploy",
        "scripts/seed-graphs.sh", "scripts/package-deployment.py",
        "scripts/import-thebrain.py",
        "scripts/vendor-runtime-assets.py",
        "scripts/build-leap-companion.ps1",
    ]
    files = []
    excluded = {"scripts/build-leap-companion.ps1", "src/bin/hyperion_bridge.rs"}

    def include(path):
        name = path.relative_to(root).as_posix()
        return not args.server_only or (
            name not in excluded and
            not name.startswith(("public/downloads/", "deploy/windows/"))
        )

    for entry in entries:
        path = root / entry
        if not include(path):
            continue
        if not path.exists():
            raise FileNotFoundError(f"Required deployment input is missing: {entry}")
        files.extend(sorted(path.rglob("*")) if path.is_dir() else [path])
    release_dir = root / "releases"
    release_dir.mkdir(exist_ok=True)
    archive = release_dir / ("meshgraph-vps-server.zip" if args.server_only else "meshgraph-vps.zip")
    with ZipFile(archive, "w", ZIP_DEFLATED) as bundle:
        for path in files:
            if path.is_file() and include(path):
                bundle.write(path, path.relative_to(root).as_posix())
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    checksum = archive.with_suffix(".zip.sha256")
    checksum.write_text(f"{digest}  {archive.name}\n", encoding="utf-8")
    print(f"Created {archive} ({archive.stat().st_size:,} bytes)")
    print(f"SHA-256: {digest}")


if __name__ == "__main__":
    main()
