"""Bundle pinned browser dependencies; never download them at app startup."""

import base64
import hashlib
import io
import json
from pathlib import Path
import tarfile
from urllib.request import urlopen


ROOT = Path(__file__).resolve().parents[1]
VENDOR = ROOT / "public/vendor"
CACHE = ROOT / "releases/vendor-download"
MODEL_URL = (
    "https://storage.googleapis.com/mediapipe-models/hand_landmarker/"
    "hand_landmarker/float16/1/hand_landmarker.task"
)


def download(url):
    with urlopen(url, timeout=180) as response:
        return response.read()


def write_asset(relative, content, source, manifest):
    path = VENDOR / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)
    manifest.append({
        "path": path.relative_to(ROOT).as_posix(),
        "source": source,
        "sha256": hashlib.sha256(content).hexdigest(),
        "bytes": len(content),
    })


def package(name, version):
    metadata = json.loads(download(f"https://registry.npmjs.org/{name}/{version}"))
    cached = CACHE / f"{version}.tgz"
    content = cached.read_bytes() if cached.exists() else download(metadata["dist"]["tarball"])
    algorithm, expected = metadata["dist"]["integrity"].split("-", 1)
    actual = base64.b64encode(hashlib.new(algorithm, content).digest()).decode("ascii")
    if actual != expected:
        raise ValueError(f"NPM archive integrity mismatch: {name}@{version}")
    cached.write_bytes(content)
    return tarfile.open(fileobj=io.BytesIO(content), mode="r:gz"), metadata["dist"]["tarball"]


def main():
    CACHE.mkdir(parents=True, exist_ok=True)
    manifest = []
    with package("aframe", "1.4.2")[0] as archive:
        for member, relative in [
            ("package/dist/aframe-v1.4.2.min.js", "aframe/aframe-1.4.2.min.js"),
            ("package/LICENSE", "aframe/LICENSE"),
        ]:
            write_asset(relative, archive.extractfile(member).read(),
                        "https://registry.npmjs.org/aframe/-/aframe-1.4.2.tgz", manifest)
    archive, source = package("@mediapipe/tasks-vision", "0.10.35")
    for filename in ["Roboto-msdf.json", "Roboto-msdf.png"]:
        url = f"https://raw.githubusercontent.com/aframevr/assets/master/fonts/{filename}"
        write_asset(f"aframe/{filename}", download(url), url, manifest)
    with archive:
        members = [m for m in archive.getmembers() if m.isfile() and (
            m.name == "package/vision_bundle.mjs" or
            (m.name.startswith("package/wasm/") and m.name.endswith((".js", ".wasm")))
        )]
        for member in members:
            relative = "mediapipe/" + member.name.removeprefix("package/")
            write_asset(relative, archive.extractfile(member).read(), source, manifest)
    write_asset("mediapipe/hand_landmarker.task", download(MODEL_URL), MODEL_URL, manifest)
    license_url = "https://www.apache.org/licenses/LICENSE-2.0.txt"
    write_asset("mediapipe/LICENSE", download(license_url), license_url, manifest)
    write_asset("aframe/Roboto-LICENSE", download(license_url), license_url, manifest)
    (VENDOR / "manifest.json").write_text(
        json.dumps({"assets": manifest}, indent=2) + "\n", encoding="utf-8"
    )
    print(f"Bundled {len(manifest)} pinned assets ({sum(a['bytes'] for a in manifest):,} bytes)")


if __name__ == "__main__":
    main()
