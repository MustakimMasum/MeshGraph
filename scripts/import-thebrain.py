"""Deterministic, standard-library TheBrain export audit and RDF converter."""
import argparse
import hashlib
import html.parser
import json
import os
from pathlib import Path
import uuid

GRAPH = "http://example.org/meshgraph/graphs/jpl-history"
NS = "http://example.org/meshgraph/history/"
ROOT_NAME = "JPL Computer Graphics Laboratory"


class NoteText(html.parser.HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.parts = []
        self.suppressed = 0

    def handle_starttag(self, tag, attrs):
        if tag in ("script", "style"):
            self.suppressed += 1
        if tag in ("p", "div", "br", "li", "h1", "h2", "h3"):
            self.parts.append("\n")

    def handle_endtag(self, tag):
        if tag in ("script", "style") and self.suppressed:
            self.suppressed -= 1
        if tag in ("p", "div", "li"):
            self.parts.append("\n")

    def handle_data(self, data):
        if not self.suppressed:
            self.parts.append(data)


def uid(value):
    return str(uuid.UUID(value))


def audit(source):
    source = source.resolve()
    hashes, warnings = {}, []

    def content(path):
        raw = path.read_bytes()
        hashes[path.relative_to(source).as_posix()] = hashlib.sha256(raw).hexdigest()
        text = raw.decode("utf-8-sig")
        if "\ufffd" in text:
            warnings.append({"kind": "encoding", "file": path.relative_to(source).as_posix()})
        return text

    def records(name):
        result = []
        for number, line in enumerate(content(source / name).splitlines(), 1):
            if line.strip():
                try:
                    row = json.loads(line)
                    if not isinstance(row, dict):
                        raise ValueError("expected an object")
                    result.append(row)
                except (ValueError, TypeError) as error:
                    raise ValueError(f"{name}:{number}: {error}") from error
        return result

    meta = records("meta.json")
    if len(meta) != 1 or meta[0].get("ExchangeFormatVersion") != 5:
        raise ValueError("meta.json: expected one exchange format version 5 record")
    brain = uid(meta[0]["BrainId"])
    thoughts, links, attachments = [records(name + ".json") for name in ("thoughts", "links", "attachments")]
    for name, rows in (("thoughts", thoughts), ("links", links), ("attachments", attachments)):
        ids = [uid(row["Id"]) for row in rows]
        if len(set(ids)) != len(ids):
            raise ValueError(f"{name}.json: duplicate IDs")
        if any(uid(row["BrainId"]) != brain for row in rows):
            raise ValueError(f"{name}.json: inconsistent BrainId")
    by_id = {t["Id"]: t for t in thoughts}
    roots = [t for t in thoughts if t.get("Name") == ROOT_NAME]
    if len(roots) != 1:
        raise ValueError("expected exactly one laboratory root")
    relation_names = {0: "Unspecified", 1: "Hierarchy", 2: "Parent", 3: "Cross-link", 4: "Sibling"}
    meaning_names = {1: "Ordinary", 2: "Type assignment", 3: "Type hierarchy", 4: "Event", 5: "Tag assignment", 6: "System", 7: "Tag hierarchy"}
    edges = []
    for link in links:
        if link.get("ThoughtIdA") not in by_id or link.get("ThoughtIdB") not in by_id:
            raise ValueError(f"links.json: dangling endpoint in {link['Id']}")
        edges.append({"id": link["Id"], "source": link["ThoughtIdA"], "target": link["ThoughtIdB"],
                      "relation": link.get("Relation", 0), "meaning": link.get("Meaning", 0),
                      "direction": link.get("Direction", -1), "name": link.get("Name") or "",
                      "kind": relation_names.get(link.get("Relation"), "Unknown relation"),
                      "meaningLabel": meaning_names.get(link.get("Meaning"), "Unknown meaning"),
                      "directionLabel": "Direction unresolved; original value retained", "weight": 1, "raw": link})
        if link.get("Direction") not in (0,):
            warnings.append({"kind": "direction", "id": link["Id"], "value": link.get("Direction")})
        if link.get("Relation") not in relation_names or link.get("Meaning") not in meaning_names:
            warnings.append({"kind": "relationship", "id": link["Id"]})
    assets, attachment_rows = {}, []
    for item in attachments:
        owner, typ = item.get("SourceId"), item.get("Type")
        if owner != brain and owner not in by_id:
            raise ValueError(f"attachments.json: unknown owner for {item['Id']}")
        location = item.get("Location") or ""
        row = {"id": item["Id"], "owner": owner, "name": item.get("Name") or location,
               "type": typ, "location": location, "status": "unavailable", "text": "", "asset": None, "raw": item}
        if typ == 3:
            row["status"] = "external-url" if location.lower().startswith(("http://", "https://")) else "unsupported-url"
        elif typ in (1, 4, 5, 6, 12):
            folder = "Notes" if typ == 4 else ".data" if typ == 5 else ""
            path = (source / owner / folder / location).resolve()
            if not path.is_relative_to(source):
                raise ValueError(f"attachments.json: path escapes export: {item['Id']}")
            if path.is_file():
                suffix = path.suffix.lower()
                if suffix in (".md", ".html", ".txt"):
                    text = content(path)
                    if suffix == ".html":
                        parser = NoteText()
                        parser.feed(text)
                        text = "\n".join(line.strip() for line in "".join(parser.parts).splitlines() if line.strip())
                    row.update(status="available", text=text)
                elif suffix in (".png", ".jpg", ".jpeg", ".webp"):
                    raw = path.read_bytes()
                    hashes[path.relative_to(source).as_posix()] = hashlib.sha256(raw).hexdigest()
                    filename = item["Id"] + suffix
                    assets[filename] = raw
                    row.update(status="available", asset=filename)
                else:
                    row["status"] = "unsupported-file"
        if row["status"] not in ("available", "external-url"):
            warnings.append({"kind": "attachment", "id": item["Id"], "status": row["status"]})
        attachment_rows.append(row)
    nodes = []
    for thought in thoughts:
        tid = thought["Id"]
        type_ids = set()
        if thought.get("TypeId") in by_id:
            type_ids.add(thought["TypeId"])
        tags = set(thought.get("TagIds") or [])
        for link in links:
            if tid not in (link["ThoughtIdA"], link["ThoughtIdB"]):
                continue
            other = link["ThoughtIdB"] if tid == link["ThoughtIdA"] else link["ThoughtIdA"]
            if link.get("Meaning") == 2 and by_id[other].get("Kind") == 2:
                type_ids.add(other)
            if link.get("Meaning") == 5 and by_id[other].get("Kind") == 4:
                tags.add(other)
        own = [a for a in attachment_rows if a["owner"] == tid]
        types = [by_id[t]["Name"] for t in sorted(type_ids)]
        tag_names = [by_id[t]["Name"] for t in sorted(tags) if t in by_id]
        if thought.get("Kind") not in (1, 2, 4, 5):
            warnings.append({"kind": "thought-kind", "id": tid, "value": thought.get("Kind")})
        raw_type = thought.get("TypeId")
        if raw_type and raw_type != "00000000-0000-0000-0000-000000000000" and raw_type not in by_id:
            warnings.append({"kind": "type-reference", "id": tid, "value": raw_type})
        nodes.append({"id": tid, "title": thought.get("Name", "Unnamed"), "kind": thought.get("Kind", 1),
                      "types": types, "tags": tag_names, "topic": ", ".join(types) or "Unclassified",
                      "year": None, "citationCount": 0, "notes": "\n\n".join(a["text"] for a in own if a["text"]), "raw": thought})
    duplicates = {}
    for node in nodes:
        duplicates.setdefault(node["title"], []).append(node["id"])
    duplicates = {name: ids for name, ids in duplicates.items() if len(ids) > 1}
    version = hashlib.sha256(json.dumps(hashes, sort_keys=True).encode()).hexdigest()
    report = {"counts": {"thoughts": len(nodes), "links": len(edges), "attachments": len(attachment_rows)},
              "danglingEndpoints": 0, "duplicateNames": duplicates, "warnings": warnings, "sourceHashes": hashes,
              "interpretation": "Direction remains unresolved. Timestamps describe source records, not historical events."}
    dataset = {"schemaVersion": 1, "version": version, "brainId": brain, "rootId": roots[0]["Id"],
               "mode": "history", "nodes": nodes, "links": edges, "attachments": attachment_rows, "quality": report, "meta": meta[0]}
    return dataset, assets


def turtle(dataset):
    literal = lambda value: json.dumps(value, ensure_ascii=True)
    lines = ["# Generated by scripts/import-thebrain.py; do not edit.", f"@prefix h: <{NS}> ."]
    lines.append(f"<{GRAPH}> h:payload {literal(json.dumps(dataset, ensure_ascii=True, sort_keys=True))} .")
    for node in dataset["nodes"]:
        lines.append(f"<urn:uuid:{node['id']}> a h:Thought ; h:name {literal(node['title'])} ; h:kind {node['kind']} .")
    for link in dataset["links"]:
        lines.append(f"<urn:uuid:{link['id']}> a h:Link ; h:source <urn:uuid:{link['source']}> ; h:target <urn:uuid:{link['target']}> ; h:relation {link['relation']} ; h:meaning {link['meaning']} ; h:direction {link['direction']} .")
    for item in dataset["attachments"]:
        lines.append(f"<urn:uuid:{item['id']}> a h:Attachment ; h:owner <urn:uuid:{item['owner']}> ; h:status {literal(item['status'])} .")
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("--output", type=Path, default=Path("data/jpl-history"))
    parser.add_argument("--assets", type=Path, default=Path("public/history-assets"))
    parser.add_argument("--validate-only", action="store_true")
    args = parser.parse_args()
    try:
        dataset, assets = audit(args.source)
        print(json.dumps({"version": dataset["version"], **dataset["quality"]["counts"], "warnings": len(dataset["quality"]["warnings"])}, indent=2))
        if args.validate_only:
            return
        args.output.mkdir(parents=True, exist_ok=True)
        args.assets.mkdir(parents=True, exist_ok=True)
        files = {args.output / "history.ttl": turtle(dataset).encode(), args.output / "report.json": json.dumps(dataset["quality"], indent=2, ensure_ascii=True).encode()}
        files.update({args.assets / name: raw for name, raw in assets.items()})
        for path, raw in files.items():
            temporary = path.with_suffix(path.suffix + ".tmp")
            temporary.write_bytes(raw)
            os.replace(temporary, path)
    except (ValueError, KeyError, OSError) as error:
        parser.exit(1, f"Import failed: {error}\n")


if __name__ == "__main__":
    main()
