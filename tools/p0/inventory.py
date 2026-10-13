"""Create a portable candidate inventory from Cargo's locked metadata (no local paths)."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tomllib


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--metadata', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    metadata = json.loads(args.metadata.read_text(encoding='utf-8'))
    locked = tomllib.loads((root / 'Cargo.lock').read_text(encoding='utf-8'))
    checksums = {(p['name'], p['version']): p.get('checksum') for p in locked['package']}
    features = {node['id']: node['features'] for node in metadata['resolve']['nodes']}
    packages = []
    for package in metadata['packages']:
        if package['source'] is None:
            continue
        record = dict(name=package['name'], version=package['version'], source=package['source'],
                      license=package['license'], checksum=checksums[(package['name'], package['version'])],
                      features=features[package['id']])
        directory = Path(package['manifest_path']).parent
        if package['name'] in ('rquickjs-sys', 'aws-lc-sys', 'cc', 'cmake'):
            notices = {}
            for path in sorted(directory.rglob('*')):
                if path.is_file() and path.name.lower().startswith(('license', 'copying', 'notice')):
                    relative = path.relative_to(directory)
                    destination = root / 'contracts/notices' / (package['name'] + '-' + package['version']) / relative
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    destination.write_bytes(path.read_bytes())
                    notices[relative.as_posix()] = dict(sha256=sha(path), bundledPath=destination.relative_to(root).as_posix())
            record['bundledNotices'] = notices
        if package['name'] in ('zen-engine', 'zen-expression', 'zen-types', 'zen-tmpl'):
            record['sourceFiles'] = {p.relative_to(directory).as_posix(): sha(p) for p in sorted(directory.rglob('*')) if p.is_file() and p.name not in ('.cargo-ok', '.cargo-checksum.json')}
        packages.append(record)
    result = dict(schemaVersion='1.0', status='candidate', lockSha256=sha(root / 'Cargo.lock'),
                  rustc=subprocess.check_output(['rustc', '-vV'], text=True),
                  cargo=subprocess.check_output(['cargo', '--version'], text=True).strip(),
                  packages=packages,
                  nativeBuildSurface=['rquickjs-sys: bundled QuickJS C sources', 'aws-lc-sys: bundled AWS-LC/OpenSSL-derived sources and assembly', 'cc/cmake build dependencies', 'platform SDK and CRT'],
                  licenseReview=dict(crateLicenseMetadata='present for all registry packages',
                                     nativeNotices='upstream native/build notice files copied verbatim and hashed; final SDK notice assembly belongs to packaging',
                                     productLicense='no SDK published by this P0; final product licensing belongs to release',
                                     currencyTable='fixed attributed SIX code/minor-unit snapshot; provenance in contracts/currency'),
                  supportedEvidence=['Windows x86_64, MSVC 19.41.34123.0, Windows 10.0.26200, dynamic CRT, C++ Release, CMake 3.29.5-msvc4'],
                  unverified=['Linux CI', 'macOS CI', 'older operating systems', 'other C++ compiler/stdlib/CRT combinations', 'production SDK/package'])
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
    print(f'inventory: {len(packages)} registry packages; candidate, not acceptance')


if __name__ == '__main__':
    main()
