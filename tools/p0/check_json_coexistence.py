"""Reproduce the JSON header inclusion gate; failed gates return nonzero.

This is a bounded dependency preflight, not the complete P0 runner.
Only --fetch-host enables a download; normal runs use the supplied local fixture.
"""

import argparse
import hashlib
import json
import platform
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import urllib.request


HOST_FILES = {
    "json.hpp": (
        "single_include/nlohmann/json.hpp",
        "9bea4c8066ef4a1c206b2be5a36302f8926f7fdc6087af5d20b417d0cf103ea6",
    ),
    "LICENSE.MIT": (
        "LICENSE.MIT",
        "86b998c792894ccb911a1cb7994f7a9652894e7a094c0b5e45be2f553f45cf14",
    ),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--host-json-dir", required=True, type=Path)
    parser.add_argument("--cmake", default="cmake")
    parser.add_argument("--fetch-host", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    out = args.out.resolve()
    host = args.host_json_dir.resolve()
    if out == root or root in out.parents:
        parser.error("build output must be outside this source worktree")
    if host == root or root in host.parents:
        parser.error("download fixture must be outside this source worktree")
    out.mkdir(parents=True, exist_ok=True)
    records = []
    evidence = {"platform": platform.platform(), "machine": platform.machine(),
                "python": platform.python_version(), "commands": records,
                "scope": "JSON inclusion preflight and C/C++ header checks only",
                "passed": False}

    def save():
        (out / "evidence.json").write_text(
            json.dumps(evidence, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
        )

    def run(name, argv):
        log = out / (name + ".log")
        try:
            with log.open("wb") as stream:
                result = subprocess.run(argv, cwd=root, stdout=stream,
                                        stderr=subprocess.STDOUT, timeout=180)
            code = result.returncode
        except subprocess.TimeoutExpired:
            records.append({"name": name, "argv": argv, "exitCode": None,
                            "timedOut": True, "log": log.name})
            save()
            print(f"{name}: timed out", flush=True)
            return 124
        records.append({"name": name, "argv": argv, "exitCode": code,
                        "timedOut": False, "log": log.name})
        save()
        print(f"{name}: exit {code}", flush=True)
        print(log.read_text(encoding="utf-8", errors="replace"), flush=True)
        return code

    try:
        evidence["hostVersion"] = "3.11.3"
        evidence["hostFiles"] = []
        for filename, (source, expected) in HOST_FILES.items():
            target = host / filename
            url = f"https://raw.githubusercontent.com/nlohmann/json/v3.11.3/{source}"
            if args.fetch_host and not target.exists():
                with urllib.request.urlopen(url, timeout=60) as response:
                    data = response.read(2 * 1024 * 1024 + 1)
                if len(data) > 2 * 1024 * 1024:
                    raise ValueError("Host fixture exceeds download bound")
                if hashlib.sha256(data).hexdigest() != expected:
                    raise ValueError(f"Downloaded {filename} hash mismatch")
                host.mkdir(parents=True, exist_ok=True)
                target.write_bytes(data)
            actual = hashlib.sha256(target.read_bytes()).hexdigest()
            if actual != expected:
                raise ValueError(f"Host {filename} hash mismatch")
            evidence["hostFiles"].append({"file": filename, "url": url, "sha256": actual})
        candidate = root / "vendor/nlohmann/provenance.json"
        evidence["bundled"] = json.loads(candidate.read_text(encoding="utf-8"))
        for item in evidence["bundled"]["files"]:
            data = (candidate.parent / item["file"]).read_bytes()
            if hashlib.sha256(data).hexdigest() != item["sha256"]:
                raise ValueError(f"Bundled {item['file']} hash mismatch")
        cmake = shutil.which(args.cmake)
        if not cmake:
            raise FileNotFoundError("CMake is unavailable; supply --cmake")
        ctest = Path(cmake).with_name("ctest.exe" if sys.platform == "win32" else "ctest")
        if not ctest.is_file():
            raise FileNotFoundError("CTest not found beside CMake")
        for name, argv in [("cmake-version", [cmake, "--version"]),
                           ("ctest-version", [str(ctest), "--version"])]:
            if run(name, argv):
                return 1
        configure = [cmake, "-S", str(root / "tests/p0/json-coexistence"),
                     "-B", str(out), f"-DHOST_JSON_DIR={host.as_posix()}"]
        if sys.platform == "win32":
            configure.extend(["-G", "Visual Studio 17 2022", "-A", "x64"])
        if run("configure", configure):
            return 1
        if run("build", [cmake, "--build", str(out), "--config", "Release", "--parallel", "2"]):
            return 1
        if sys.platform == "win32":
            evidence["binaryMachines"] = {}
            for name in ("host_first", "bundled_first", "host_diagnostics_first",
                         "separate_translation_units", "abi_c", "abi_cpp"):
                with (out / "Release" / (name + ".exe")).open("rb") as executable:
                    if executable.read(2) != b"MZ":
                        raise ValueError("Missing PE DOS signature")
                    executable.seek(0x3C)
                    offset = struct.unpack("<I", executable.read(4))[0]
                    executable.seek(offset)
                    if executable.read(4) != b"PE\0\0":
                        raise ValueError("Missing PE signature")
                    machine = struct.unpack("<H", executable.read(2))[0]
                evidence["binaryMachines"][name] = hex(machine)
                if machine != 0x8664:
                    raise ValueError(f"{name}: expected an x64 PE binary")
            evidence["machine"] = "x86_64 (verified PE machine 0x8664)"
        code = run("ctest", [str(ctest), "--test-dir", str(out), "-C", "Release",
                            "--output-on-failure", "--output-junit", str(out / "results.xml")])
        evidence["passed"] = code == 0
        save()
        return 0 if code == 0 else 1
    except (OSError, ValueError) as exc:
        evidence["error"] = str(exc)
        save()
        print(str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
