import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("importer", Path(__file__).parents[1] / "scripts/import-thebrain.py")
importer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(importer)

BRAIN = "10000000-0000-0000-0000-000000000000"
ROOT = "20000000-0000-0000-0000-000000000000"
OTHER = "30000000-0000-0000-0000-000000000000"
LINK = "40000000-0000-0000-0000-000000000000"
ATTACH = "50000000-0000-0000-0000-000000000000"


class ImportTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name)
        self.write("meta", [{"BrainId": BRAIN, "ExchangeFormatVersion": 5}])
        self.write("thoughts", [{"Id": ROOT, "BrainId": BRAIN, "Name": importer.ROOT_NAME, "Kind": 1}, {"Id": OTHER, "BrainId": BRAIN, "Name": "A topic", "Kind": 1}])
        self.write("links", [{"Id": LINK, "BrainId": BRAIN, "ThoughtIdA": ROOT, "ThoughtIdB": OTHER, "Relation": 3, "Meaning": 1, "Direction": -1}])
        self.write("attachments", [])

    def write(self, name, rows):
        (self.path / (name + ".json")).write_text("\n".join(json.dumps(row) for row in rows), encoding="utf-8")

    def test_repeated_import_is_deterministic_and_preserves_unknown_direction(self):
        first, _ = importer.audit(self.path)
        second, _ = importer.audit(self.path)
        self.assertEqual(importer.turtle(first), importer.turtle(second))
        self.assertEqual(first["links"][0]["raw"]["Direction"], -1)
        self.assertIsNone(first["nodes"][0]["year"])

    def test_bad_json_reports_filename_and_line(self):
        with (self.path / "thoughts.json").open("a") as f:
            f.write("\n{broken")
        with self.assertRaisesRegex(ValueError, "thoughts.json:3"):
            importer.audit(self.path)

    def test_rejects_dangling_links(self):
        self.write("links", [{"Id": LINK, "BrainId": BRAIN, "ThoughtIdA": ROOT, "ThoughtIdB": ATTACH}])
        with self.assertRaisesRegex(ValueError, "dangling endpoint"):
            importer.audit(self.path)

    def test_rejects_attachment_path_escape(self):
        self.write("attachments", [{"Id": ATTACH, "BrainId": BRAIN, "SourceId": ROOT, "Type": 1, "Location": "../../outside.txt"}])
        with self.assertRaisesRegex(ValueError, "escapes export"):
            importer.audit(self.path)

    def test_html_notes_are_readable_text_and_external_files_are_not_read(self):
        folder = self.path / ROOT / "Notes"
        folder.mkdir(parents=True)
        (folder / "notes.html").write_text('<p>A &amp; B</p><script>secret()</script><p>Second</p>', encoding="utf-8")
        self.write("attachments", [{"Id": ATTACH, "BrainId": BRAIN, "SourceId": ROOT, "Type": 4, "Location": "notes.html"}, {"Id": "60000000-0000-0000-0000-000000000000", "BrainId": BRAIN, "SourceId": BRAIN, "Type": 2, "Location": "C:/private.txt"}])
        dataset, _ = importer.audit(self.path)
        self.assertEqual(dataset["nodes"][0]["notes"], "A & B\nSecond")
        self.assertEqual(dataset["attachments"][1]["status"], "unavailable")


if __name__ == "__main__":
    unittest.main()
