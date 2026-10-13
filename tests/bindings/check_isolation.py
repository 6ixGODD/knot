import argparse
import json
from pathlib import Path
import re
import subprocess
import sys

p = argparse.ArgumentParser()
p.add_argument('--build-dir', type=Path, required=True)
p.add_argument('--config', required=True)
p.add_argument('--cmake', required=True)
a = p.parse_args()
fixture = json.loads((a.build_dir / f'fixtures/{a.config}.json').read_text())
for name in ('sharedBridge', 'staticBridge'):
    lib = fixture[name]
    cmd = [fixture['linker'], '/dump', '/symbols', lib] if sys.platform == 'win32' else [fixture['nm'], '-C', lib]
    result = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    output = result.stdout.decode(errors='replace')
    (a.build_dir / f'{name}-symbols.log').write_text(output, encoding='utf-8')
    if result.returncode or 'knot_private_json_v3_12_0' not in output or re.search(r'@nlohmann@|\bnlohmann::|8nlohmann', output):
        raise SystemExit(f'{name}: JSON isolation failed')
    if sys.platform == 'win32':
        if ('__imp_knot_model_create' in output) != (name == 'sharedBridge'):
            raise SystemExit('Incorrect DLL/static bridge reference')
for name, marker in [('diagnostics','JSON_DIAGNOSTICS'),('implicit','JSON_USE_IMPLICIT_CONVERSIONS'),
                     ('namespace','NLOHMANN_JSON_NAMESPACE_NO_VERSION'),('preincluded','INCLUDE_NLOHMANN_JSON_HPP_'),('pch','INCLUDE_NLOHMANN_JSON_HPP_')]:
    result = subprocess.run([a.cmake,'--build',str(a.build_dir),'--config',a.config,'--target','reject_'+name],stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    output = result.stdout.decode(errors='replace')
    (a.build_dir / f'reject-{name}.log').write_text(output, encoding='utf-8')
    if result.returncode == 0 or 'KNOT_JSON_POLLUTION: '+marker not in output:
        raise SystemExit(f'Expected pollution refusal missing: {name}')
print('PASS: both archive namespaces/linkage and five actual codec pollution refusals')
