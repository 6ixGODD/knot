# P0 native dependency notices

The files in the package/version directories are copied verbatim from the locked
Cargo source archives. `p0-dependencies.json` records their original relative
paths and SHA-256 values, together with package checksums and SPDX metadata.

QuickJS uses MIT; AWS-LC supplies its combined upstream notices (including its
BoringSSL/OpenSSL-derived and fiat components); cc and cmake provide their
MIT/Apache-2.0 notices. These files preserve the original attribution. They are
not a replacement license for KNOT, nor a claim that every locked package is
linked into every target. The inventory includes target-specific and build-only
dependencies. The final SDK packager must carry applicable notices alongside
its actual binaries, including the existing nlohmann notice.
