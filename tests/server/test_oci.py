"""Reject corrupt OCI graphs and attestations before importing a candidate."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools/server"))
from oci import inspect_layout


class OciIntegrity(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(dir=OUTPUT)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "blobs/sha256").mkdir(parents=True)

    def blob(self, value, media):
        data = json.dumps(value, sort_keys=True).encode()
        digest = hashlib.sha256(data).hexdigest()
        (self.root / "blobs/sha256" / digest).write_bytes(data)
        return dict(mediaType=media, digest="sha256:" + digest, size=len(data))

    def layout(self, *, subject=True, provenance=True):
        config_type = "application/vnd.oci.image.config.v1+json"
        manifest_type = "application/vnd.oci.image.manifest.v1+json"
        index_type = "application/vnd.oci.image.index.v1+json"
        config = self.blob(dict(rootfs=dict(diff_ids=[]), config=dict(User="10001:10001")), config_type)
        runtime = self.blob(dict(schemaVersion=2, config=config, layers=[]), manifest_type)
        runtime["platform"] = dict(os="linux", architecture="amd64")
        layers = []
        predicates = ["https://spdx.dev/Document"]
        if provenance:
            predicates.append("https://slsa.dev/provenance/v0.2")
        for predicate in predicates:
            layers.append(self.blob(dict(predicateType=predicate, subject=[dict(digest=dict(sha256=runtime["digest"].split(":")[1] if subject else "0" * 64))]), "application/vnd.in-toto+json"))
        attestation = self.blob(dict(schemaVersion=2, config=self.blob({}, config_type), layers=layers), manifest_type)
        attestation["platform"] = dict(os="unknown", architecture="unknown")
        top = self.blob(dict(schemaVersion=2, manifests=[runtime, attestation]), index_type)
        (self.root / "index.json").write_text(json.dumps(dict(schemaVersion=2, manifests=[top])))
        (self.root / "oci-layout").write_text('{"imageLayoutVersion":"1.0.0"}')
        return config

    def test_complete_graph(self):
        config = self.layout()
        self.assertEqual(inspect_layout(self.root)["configDigest"], config["digest"])

    def test_corrupt_blob(self):
        config = self.layout()
        (self.root / "blobs/sha256" / config["digest"].split(":")[1]).write_bytes(b"corrupt")
        with self.assertRaisesRegex(ValueError, "descriptor mismatch"):
            inspect_layout(self.root)

    def test_wrong_descriptor_size(self):
        self.layout()
        path = self.root / "index.json"
        index = json.loads(path.read_text())
        index["manifests"][0]["size"] += 1
        path.write_text(json.dumps(index))
        with self.assertRaisesRegex(ValueError, "descriptor mismatch"):
            inspect_layout(self.root)

    def test_wrong_subject(self):
        self.layout(subject=False)
        with self.assertRaisesRegex(ValueError, "subject mismatch"):
            inspect_layout(self.root)

    def test_missing_provenance(self):
        self.layout(provenance=False)
        with self.assertRaisesRegex(ValueError, "missing SBOM or provenance"):
            inspect_layout(self.root)

    def test_unreferenced_blob(self):
        self.layout()
        self.blob(dict(unreferenced=True), "application/json")
        with self.assertRaisesRegex(ValueError, "unreferenced"):
            inspect_layout(self.root)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True, type=Path)
    OUTPUT = parser.parse_args().out.resolve()
    OUTPUT.mkdir(parents=True, exist_ok=True)
    unittest.main(argv=[sys.argv[0]])
