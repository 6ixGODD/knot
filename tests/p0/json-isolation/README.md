# Private JSON codec P0 probe

This test-only codec checks the approved compiled C++ boundary. It is not a
production wrapper, Rust backend, C ABI probe, or distributable SDK.
The original `json-coexistence` directory remains the failing direct-inclusion
reproduction; its expected results have not been changed.

Run from the source worktree, with output and fixtures outside that worktree:

```text
python -B tools/p0/check_json_isolation.py --out <plan-build>/json-isolation-resume --host-json-dir <plan-build>/json-host
```

Use `--cmake <executable>` if CMake is not on PATH. The host directory contains
the pinned 3.11.3 header and license already downloaded by
`check_json_coexistence.py --fetch-host`. This runner never downloads anything.
On Windows it selects Visual Studio 2022 x64 Release. Other platforms use the
configured native CMake generator; Linux/macOS validation belongs in CI.

The runner requires all of these checks:

- Nine real parse consumers: no host JSON, then host 3.11.3/3.12.0 with and
  without diagnostics, each in both public-header inclusion orders. All host
  variants enable implicit conversion while the private codec disables it.
  Malformed syntax, a missing field, and a wrong field type must become the
  public probe error, never a private JSON exception.
- Five compile rejections: diagnostic macro, implicit-conversion macro,
  namespace macro, a simulated unity preinclude, and an actual generated PCH.
  A nonzero compiler exit counts only with the expected pollution diagnostic.
- Static archive symbol inspection: private JSON namespace present and
  original JSON namespace absent. Missing symbol tools fail the gate.

The codec rejects JSON-related macros found in the exact pinned vendor source
before including it, fixes its own configuration, and renames the outer token
only in its private TU. The vendor source is unchanged. This adaptation must be
re-audited on upgrade; upstream namespace macros alone do not promise it works.
The codec disables PCH and unity even when project-wide unity and a target PCH
are requested. Negative targets deliberately violate that setup to test the
source-level rejection. No JSON include paths or definitions are transitive.

`evidence.json`, per-command logs, JUnit results, and the generated toolchain
record identify actual inputs and outcomes. The runner fails on timeouts,
missing tools, fixture changes, skipped/absent tests, and unexpected diagnostics.
It does not prove real C-core dynamic/static bridge linkage, CRT compatibility,
LTO, relocated package consumption, minimum OS support, or production ownership
and error semantics. Those remain required P0/integration work.

Source references (read 2026-10-13):
[upstream namespace limits](https://json.nlohmann.me/features/namespace/),
[CMake unity](https://cmake.org/cmake/help/latest/prop_tgt/UNITY_BUILD.html),
[CMake PCH exclusion](https://cmake.org/cmake/help/latest/prop_tgt/DISABLE_PRECOMPILE_HEADERS.html).
