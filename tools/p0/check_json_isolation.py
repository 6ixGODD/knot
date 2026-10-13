"""Validate the private JSON codec boundary; not the complete P0 or SDK gate.

Offline: reuse the pinned host fixture fetched by check_json_coexistence.py.
The original failed coexistence probes and their expectations remain unchanged.
"""

import argparse
import hashlib
import json
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import xml.etree.ElementTree as ET

sys.dont_write_bytecode = True
from check_json_coexistence import HOST_FILES


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--host-json-dir", type=Path, required=True)
    parser.add_argument("--cmake", default="cmake")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    out = args.out.resolve()
    host = args.host_json_dir.resolve()
    if out == root or root in out.parents:
        parser.error("build output must be outside this source worktree")
    out.mkdir(parents=True, exist_ok=True)
    evidence = {
        "scope": "test-only JSON codec; no Rust/C ABI, production SDK or package validation",
        "platform": platform.platform(), "machine": platform.machine(),
        "python": platform.python_version(), "configuration": "Release",
        "passed": False, "commands": [], "negativeChecks": [],
        "limitations": ["No real native core or shared/static C++ bridge variants",
                        "No LTO, relocated package or other-platform validation"],
    }

    def save():
        (out / "evidence.json").write_text(
            json.dumps(evidence, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

    def run(name, argv):
        log = out / (name + ".log")
        code = None
        timed_out = False
        try:
            with log.open("wb") as stream:
                code = subprocess.run(argv, cwd=root, stdout=stream,
                                      stderr=subprocess.STDOUT, timeout=180).returncode
        except subprocess.TimeoutExpired:
            timed_out = True
        evidence["commands"].append({"name": name, "argv": argv, "exitCode": code,
                                     "timedOut": timed_out, "log": log.name})
        save()
        print(f"{name}: exit {code}, timedOut={timed_out}", flush=True)
        if timed_out:
            raise RuntimeError(f"{name} timed out; not a passing check")
        return code, log.read_text(encoding="utf-8", errors="replace")

    def require_success(name, argv):
        code, output = run(name, argv)
        if code != 0:
            raise RuntimeError(f"{name} failed with exit {code}; see {name}.log")
        return output

    try:
        evidence["hostFiles"] = {}
        for name, (_, expected) in HOST_FILES.items():
            actual = digest(host / name)
            if actual != expected:
                raise ValueError(f"Host {name} hash mismatch")
            evidence["hostFiles"][name] = actual
        provenance_path = root / "vendor/nlohmann/provenance.json"
        evidence["bundled"] = json.loads(provenance_path.read_text(encoding="utf-8"))
        for item in evidence["bundled"]["files"]:
            if digest(provenance_path.parent / item["file"]) != item["sha256"]:
                raise ValueError(f"Bundled {item['file']} hash mismatch")
        source = root / "tests/p0/json-isolation"
        inputs = list(source.iterdir()) + [Path(__file__).resolve(),
                                         root / "tools/p0/check_json_coexistence.py"]
        evidence["sourceSha256"] = {path.relative_to(root).as_posix(): digest(path)
                                    for path in sorted(inputs) if path.is_file()}
        cmake = shutil.which(args.cmake)
        if not cmake:
            raise FileNotFoundError("CMake unavailable; supply --cmake")
        ctest = Path(cmake).with_name("ctest.exe" if sys.platform == "win32" else "ctest")
        require_success("cmake-version", [cmake, "--version"])
        build = out / "build"
        configure = [cmake, "-S", str(source), "-B", str(build),
                     f"-DHOST_JSON_DIR={host.as_posix()}", "-DCMAKE_BUILD_TYPE=Release",
                     "-DCMAKE_UNITY_BUILD=ON"]
        if sys.platform == "win32":
            configure += ["-G", "Visual Studio 17 2022", "-A", "x64"]
        require_success("configure", configure)
        require_success("build", [cmake, "--build", str(build), "--config", "Release",
                                  "--parallel", "2"])
        evidence["toolchain"] = (build / "toolchain.txt").read_text(encoding="utf-8")
        require_success("ctest", [str(ctest), "--test-dir", str(build), "-C", "Release",
                                  "--verbose", "--output-junit", str(out / "results.xml")])
        cases = ET.parse(out / "results.xml").getroot().findall(".//testcase")
        if len(cases) != 9 or any(case.find(tag) is not None for case in cases
                                 for tag in ("failure", "error", "skipped")):
            raise ValueError("Expected exactly nine executed, passing runtime cases")
        evidence["runtimeTests"] = {"total": len(cases), "passed": len(cases), "skipped": 0}
        for name, marker in [("diagnostics", "JSON_DIAGNOSTICS"),
                             ("implicit", "JSON_USE_IMPLICIT_CONVERSIONS"),
                             ("namespace", "NLOHMANN_JSON_NAMESPACE_NO_VERSION"),
                             ("preincluded", "INCLUDE_NLOHMANN_JSON_HPP_"),
                             ("pch", "INCLUDE_NLOHMANN_JSON_HPP_")]:
            code, output = run("reject-" + name, [cmake, "--build", str(build),
                               "--config", "Release", "--target", "reject_" + name])
            passed = code != 0 and "P0_JSON_POLLUTION: " + marker in output
            evidence["negativeChecks"].append({"name": name, "exitCode": code,
                                                "requiredDiagnostic": marker, "passed": passed})
            if not passed:
                raise ValueError(f"{name} did not fail with the required pollution diagnostic")
        artifacts = (build / "artifacts-Release.txt").read_text(encoding="utf-8").splitlines()
        archive = Path(artifacts[0])
        if sys.platform == "win32":
            argv = [artifacts[1], "/dump", "/symbols", str(archive)]
        else:
            nm = artifacts[2] if len(artifacts) > 2 else shutil.which("nm")
            if not nm:
                raise FileNotFoundError("No symbol inspection tool available")
            argv = [nm, "-C", str(archive)]
        symbols = require_success("archive-symbols", argv)
        private_found = "knot_private_json_v3_12_0" in symbols
        original_found = re.search(r"@nlohmann@|\bnlohmann::|8nlohmann", symbols) is not None
        evidence["archive"] = {"path": str(archive), "sha256": digest(archive),
                               "privateNamespaceFound": private_found,
                               "originalNamespaceFound": original_found}
        if not private_found or original_found:
            raise ValueError("Static archive lacks private JSON symbols or leaks original namespace")
        evidence["passed"] = True
        save()
        print("PASS: 9 runtime tests, 5 pollution rejections, static archive isolation")
        return 0
    except (OSError, ValueError, RuntimeError, ET.ParseError) as error:
        evidence["error"] = str(error)
        save()
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
