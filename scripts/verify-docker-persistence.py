"""Verify the running Compose stack without replacing its graphs.

Requires Python 3.10+, Docker Compose, and the gateway on localhost:3000.
With --restart, briefly restarts/stops the database and verifies recovery.
"""
import argparse
import hashlib
import json
import subprocess
import time
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def compose(*args):
    subprocess.run(["docker", "compose", *args], cwd=ROOT, check=True)


def get_json(base, route):
    with urllib.request.urlopen(base + route, timeout=10) as response:
        return json.load(response)


def digest(records):
    normalized = sorted(json.dumps(item, sort_keys=True) for item in records)
    return hashlib.sha256("\n".join(normalized).encode()).hexdigest()


def snapshot(base):
    history = get_json(base, "/api/v1/history/graph")
    citations = get_json(base, "/api/v1/citations/graph")
    if not history["nodes"] or not citations["nodes"]:
        raise RuntimeError("Both collections must contain nodes")
    return {
        "historyVersion": history["version"],
        "historyCounts": [len(history[key]) for key in ("nodes", "links", "attachments")],
        "historyDigest": digest(history["nodes"] + history["links"] + history["attachments"]),
        "citationCounts": [len(citations[key]) for key in ("nodes", "links")],
        "citationDigest": digest(citations["nodes"] + citations["links"]),
    }


def wait_for_snapshot(base):
    deadline = time.monotonic() + 60
    last_error = None
    while time.monotonic() < deadline:
        try:
            return snapshot(base)
        except (OSError, ValueError, RuntimeError) as error:
            last_error = error
            time.sleep(1)
    raise RuntimeError(f"Gateway/collections did not recover: {last_error}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-url", default="http://127.0.0.1:3000")
    parser.add_argument("--restart", action="store_true", help="Restart and stop/start Oxigraph")
    parser.add_argument("--check-seed", action="store_true", help="Verify normal seeding preserves data")
    parser.add_argument("--output", type=Path, default=ROOT / "demo/jpl-history/docker-persistence.json")
    args = parser.parse_args()
    for service in ("oxigraph_db", "axum_gateway"):
        running = subprocess.check_output(
            ["docker", "compose", "ps", "--status", "running", "-q", service],
            cwd=ROOT, text=True,
        ).strip()
        if not running:
            raise RuntimeError(f"Compose service {service} is not running")
    before = wait_for_snapshot(args.base_url.rstrip("/"))
    report = {"date": datetime.now(timezone.utc).isoformat(), "before": before, "checks": []}
    try:
        actions = []
        if args.restart:
            actions.extend([
                ("restart", [("restart", "oxigraph_db")]),
                ("stop/start", [("stop", "oxigraph_db"), ("start", "oxigraph_db")]),
            ])
        if args.check_seed:
            actions.append(("repeat seed", [("run", "--rm", "-e", "FORCE_SEED=false", "graph_seed")]))
        for name, commands in actions:
            print(f"Checking {name}", flush=True)
            for command in commands:
                compose(*command)
            after = wait_for_snapshot(args.base_url.rstrip("/"))
            passed = after == before
            report["checks"].append({"action": name, "passed": passed, "after": after})
            if not passed:
                raise RuntimeError(f"Collection content changed after {name}")
        report["passed"] = True
    except Exception as error:
        report["passed"] = False
        report["error"] = str(error)
        raise
    finally:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Verified collection content; report: {args.output}")


if __name__ == "__main__":
    main()
