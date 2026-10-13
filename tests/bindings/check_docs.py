"""Check public links, exact compiled snippets, and real Rust Schema validation."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
from urllib.parse import unquote
from doc_tables import field_table, limits_table

def main():
    p=argparse.ArgumentParser()
    p.add_argument('--contracts',type=Path,required=True)
    p.add_argument('--out',type=Path,required=True)
    a=p.parse_args()
    root=Path(__file__).resolve().parents[2]
    contracts=a.contracts.resolve()
    out=a.out.resolve();out.mkdir(parents=True,exist_ok=True)
    if not os.environ.get('CARGO_TARGET_DIR'): raise ValueError('Explicit plan-isolated CARGO_TARGET_DIR required')
    pages=[root/'README.md']+sorted((root/'docs').glob('*.md'))
    required={'quickstart','integration','c-api','cpp-api','dsl-reference','state-machine','pricing','visual-ir','limits','troubleshooting'}
    if not required.issubset({p.stem for p in pages}):raise ValueError('Missing public reference page')
    evidence=[];snippets=0;json_cases=0
    for page in pages:
        content=page.read_text(encoding='utf-8')
        if re.search(r'docs/(?:design|plan)|\.mine/|AGENTS\.md|REQUIREMENTS\.md',content):raise ValueError(f'Internal path in {page}')
        prose=re.sub(r'```[^\n]*\n.*?\n```','',content,flags=re.S)
        for link in re.findall(r'\]\(([^)]+)\)',prose):
            if re.match(r'https?://',link) or link.startswith('#'):continue
            target=unquote(link.split('#')[0])
            if not (page.parent/target).resolve().exists():raise ValueError(f'Broken link {page.name}: {link}')
        for source,body in re.findall(r'<!-- source: (.*?) -->\s*```[^\n]*\n(.*?)\n```',content,re.S):
            actual=(root/source).read_text(encoding='utf-8').rstrip()
            if body!=actual:raise ValueError(f'Snippet drift: {source}')
            snippets+=1
        fences=list(re.finditer(r'```json\n(.*?)\n```',content,re.S))
        for match in fences:
            annotation=re.search(r'<!-- schema: (\w+) expect: (valid|invalid) -->\s*$',content[:match.start()])
            if not annotation:raise ValueError(f'Unlabelled JSON in {page}')
            kind,expected=annotation.groups();json_cases+=1
            sample=out/f'{page.stem}-{json_cases}.json';sample.write_text(match.group(1),encoding='utf-8')
            command=['cargo','run','-p','knot-schema','--example','validate_json','--locked','--','--schema',str(contracts/f'schemas/{kind}.schema.json'),'--input',str(sample)]
            r=subprocess.run(command,cwd=root,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
            (out/f'{sample.stem}.stdout').write_bytes(r.stdout);(out/f'{sample.stem}.stderr').write_bytes(r.stderr)
            evidence.append({'page':page.name,'command':command,'expected':expected,'exitCode':r.returncode})
            (out/'evidence.json').write_text(json.dumps(evidence,indent=2),encoding='utf-8')
            # An invalid JSON example must fail in the validator, not in Cargo.
            response=json.loads(r.stdout)
            if (expected=='valid' and r.returncode!=0) or (expected=='invalid' and (r.returncode!=1 or not response.get('error'))):
                raise ValueError(f'Schema validation did not match expectation: {sample}')
    for filename,table in [('dsl-reference.md',field_table(json.loads((contracts/'schemas/product.schema.json').read_text()))),
                           ('limits.md',limits_table(json.loads((contracts/'limits.json').read_text())))]:
        if table not in (root/'docs'/filename).read_text(encoding='utf-8'):raise ValueError(f'Contract table drift: {filename}')
    if snippets<2 or json_cases<2:raise ValueError('Missing executable snippets/Schema examples')
    print(f'PASS: {len(pages)} pages, {snippets} exact source snippets, {json_cases} real Schema checks')

if __name__=='__main__':main()
