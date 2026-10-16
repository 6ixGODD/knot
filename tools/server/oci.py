"""Validate the complete OCI descriptor graph and identify runtime attestations."""
import hashlib
import json
import re
import tomllib
from pathlib import Path
from urllib.parse import unquote


def inspect_layout(directory, cargo_lock=None):
    directory = Path(directory)
    seen = {}
    def blob(descriptor):
        digest = descriptor["digest"]
        if not re.fullmatch(r"sha256:[0-9a-f]{64}", digest):
            raise ValueError("unsupported OCI digest")
        path = directory / "blobs/sha256" / digest.split(":")[1]
        with path.open("rb") as stream:
            actual = hashlib.file_digest(stream, "sha256").hexdigest()
        if actual != digest.split(":")[1] or path.stat().st_size != descriptor["size"]:
            raise ValueError("OCI descriptor mismatch")
        seen[digest] = descriptor["size"]
        return path
    def visit(descriptor):
        first = descriptor["digest"] not in seen
        path = blob(descriptor)
        if not first:
            return
        media = descriptor.get("mediaType", "")
        if "manifest" not in media and "index" not in media:
            return
        obj = json.loads(path.read_text())
        for child in obj.get("manifests", []) + obj.get("layers", []) + ([obj["config"]] if "config" in obj else []) + ([obj["subject"]] if "subject" in obj else []):
            visit(child)
    if json.loads((directory / "oci-layout").read_text()).get("imageLayoutVersion") != "1.0.0":
        raise ValueError("unsupported OCI layout version")
    index = json.loads((directory / "index.json").read_text())
    if index.get("schemaVersion") != 2:
        raise ValueError("unsupported OCI index version")
    if len(index["manifests"]) != 1:
        raise ValueError("expected one OCI layout image entry")
    top = index["manifests"][0]
    visit(top)
    image_index = json.loads(blob(top).read_text())
    runtime = [d for d in image_index.get("manifests", []) if d.get("platform", {}).get("os") == "linux" and d.get("platform", {}).get("architecture") == "amd64"]
    if len(runtime) != 1:
        raise ValueError("expected exactly one Linux amd64 runtime")
    manifest = json.loads(blob(runtime[0]).read_text())
    config = json.loads(blob(manifest["config"]).read_text())
    predicates = []
    cargo_packages = set()
    for entry in image_index["manifests"]:
        if entry["digest"] == runtime[0]["digest"]:
            continue
        attestation = json.loads(blob(entry).read_text())
        for layer in attestation.get("layers", []):
            statement = json.loads(blob(layer).read_text())
            predicate = statement.get("predicateType", "")
            if not any(s.get("digest", {}).get("sha256") == runtime[0]["digest"].split(":")[1] for s in statement.get("subject", [])):
                raise ValueError("attestation subject mismatch")
            predicates.append(predicate)
            if predicate == "https://spdx.dev/Document":
                for package in statement.get("predicate", {}).get("packages", []):
                    for reference in package.get("externalRefs", []):
                        locator = reference.get("referenceLocator", "")
                        if locator.startswith("pkg:cargo/") and "@" in locator:
                            name, version = locator[len("pkg:cargo/"):].split("@", 1)
                            cargo_packages.add((unquote(name), unquote(version.split("?", 1)[0])))
    if not any("spdx" in p for p in predicates) or not any("slsa.dev/provenance" in p for p in predicates):
        raise ValueError("missing SBOM or provenance")
    if cargo_lock is not None:
        expected = {(p["name"], p["version"]) for p in tomllib.loads(Path(cargo_lock).read_text())["package"] if "source" in p}
        if expected - cargo_packages:
            raise ValueError("SBOM missing locked Cargo packages: " + repr(sorted(expected - cargo_packages)))
    for path in (directory / "blobs/sha256").iterdir():
        if not path.is_file() or "sha256:" + path.name not in seen:
            raise ValueError("unreferenced or unsupported OCI blob")
    return dict(indexDigest=top["digest"], runtimeManifestDigest=runtime[0]["digest"], configDigest=manifest["config"]["digest"], runtimeConfig=config, predicates=predicates, verifiedBlobs=seen, cargoSbomPackages=len(cargo_packages))
