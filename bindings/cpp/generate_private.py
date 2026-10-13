"""Generate private build inputs from the single frozen vendor/contracts copies."""
import hashlib
import json
from pathlib import Path
import re
import sys

root, out = map(Path, sys.argv[1:])
out.mkdir(parents=True, exist_ok=True)
vendor = (root / 'vendor/nlohmann/json.hpp').read_bytes()
if hashlib.sha256(vendor).hexdigest() != 'aaf127c04cb31c406e5b04a63f1ae89369fccde6d8fa7cdda1ed4f32dfc5de63':
    raise SystemExit('Pinned vendor digest mismatch')
macros = sorted(set(re.findall(r'(?:JSON_|NLOHMANN_|INCLUDE_NLOHMANN_)[A-Z0-9_]+', vendor.decode())) | {'nlohmann'})
macros.remove('INCLUDE_NLOHMANN_JSON_HPP_')
macros.insert(0, 'INCLUDE_NLOHMANN_JSON_HPP_')
(out / 'reject_json_pollution.hpp').write_text(''.join(f'#ifdef {m}\n#error KNOT_JSON_POLLUTION: {m}\n#endif\n' for m in macros))
supported = {'$schema', '$id', '$defs', '$ref', 'type', 'const', 'enum', 'oneOf', 'anyOf', 'allOf', 'if', 'then', 'else',
             'properties', 'required', 'additionalProperties', 'propertyNames', 'maxProperties', 'items', 'minItems',
             'maxItems', 'uniqueItems', 'minLength', 'maxLength', 'pattern', 'format', 'minimum', 'maximum', 'default'}
def audit(schema):
    if isinstance(schema, bool):
        return
    if not isinstance(schema, dict) or set(schema) - supported:
        raise ValueError(f'Unsupported schema vocabulary: {schema}')
    if '$ref' in schema and not schema['$ref'].startswith('#/'):
        raise ValueError('External schema reference')
    for k in ('$defs', 'properties'):
        for v in schema.get(k, {}).values(): audit(v)
    for k in ('oneOf', 'anyOf', 'allOf'):
        for v in schema.get(k, []): audit(v)
    for k in ('items', 'if', 'then', 'else', 'additionalProperties', 'propertyNames'):
        if k in schema: audit(schema[k])
schemas = {}
for name in ('result', 'state', 'event', 'error'):
    schemas[name] = json.loads((root / f'contracts/schemas/{name}.schema.json').read_text())
    audit(schemas[name])
schemas['statuses'] = {e['code']: e['status'] for e in json.loads((root / 'contracts/errors.json').read_text())['errors']}
payload = json.dumps(schemas, ensure_ascii=True, separators=(',', ':'))
# Split literals for MSVC's individual string-literal size limit.
chunks = [json.dumps(payload[i:i+8000]) for i in range(0, len(payload), 8000)]
(out / 'wire_schemas.hpp').write_text('static constexpr char wire_schemas[] =\n' + '\n'.join(chunks) + ';\n')
