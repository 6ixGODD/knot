# JSON dependency inclusion preflight

This diagnostic exercises unmodified nlohmann/json 3.12.0 (bundled candidate)
and 3.11.3 (host fixture). It is not a product wrapper or a complete SDK test.
The host fixture stays outside the source tree; exact hashes and MIT notices
are verified by the runner. The bundled candidate has its own provenance file.

Run from the repository checkout, using output and fixture directories outside
that checkout:

```text
python tools/p0/check_json_coexistence.py --out <build-dir> --host-json-dir <fixture-dir> --fetch-host --cmake <cmake-executable>
```

Omit `--fetch-host` for a fully offline rerun. It only downloads the pinned host
header and license if absent, verifies their hashes before saving, and never
changes an existing file. The build uses CMake, C11, and C++17. On Windows it
selects Visual Studio 2022 x64 and verifies every resulting PE machine field.
The JSON evidence records exact commands, return codes, tool versions, input
hashes, and paths to raw logs. A failed gate returns nonzero, never green.

| Test | Required property | Windows result (2026-10-12) |
|---|---|---|
| host_first | Host retains 3.11.3; bundled include supplies 3.12.0 | FAIL: both see minor 11 |
| bundled_first | Bundled retains 3.12.0; host include supplies 3.11.3 | FAIL: both see minor 12 |
| host_diagnostics_first | Same-version host diagnostics=1 and bundled diagnostics=0 remain isolated | FAIL: both types are 24 bytes |
| separate_translation_units | Distinct source files with distinct versions/configurations can parse without exposing JSON types across their boundary | PASS: 53 / 54 |
| abi_c | Independent C compilation of the contract header, widths and selected offsets | PASS |
| abi_cpp | Independent C++17 compilation, standard layout and all public function signatures | PASS |

Both original headers use `INCLUDE_NLOHMANN_JSON_HPP_`; the first include wins.
Changing `JSON_DIAGNOSTICS` after inclusion changes the macro but does not
redefine the already-created type. The tests deliberately retain failure
status: silently selecting the host's dependency does not prove use of the
fixed bundled version/configuration.

The separate-translation-unit control shows that the compiler and both
libraries work individually, and demonstrates one supported isolation shape.
It does not establish that every possible wrapper layout or ODR interaction
is safe. This experiment disproves isolation by ordinary direct inclusion;
it does not prove that every possible header transformation is impossible.
No header guard was removed, no namespace was rewritten, and no alternative
SDK delivery policy was silently implemented.

Relevant upstream sources:

- https://json.nlohmann.me/features/namespace/
- https://raw.githubusercontent.com/nlohmann/json/v3.12.0/single_include/nlohmann/json.hpp
- https://raw.githubusercontent.com/nlohmann/json/v3.11.3/single_include/nlohmann/json.hpp

The ABI checks only compile the header. They do not verify a Rust ABI
implementation, library exports, ownership/free behavior, or dynamic/static
SDK consumption; no KNOT product symbols are implemented by these probes.
