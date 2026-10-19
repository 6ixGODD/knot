"""Render auditable public field/limit tables from frozen contracts."""
import json

def field_table(schema):
    rows = ['| Field / variant | Type / values | Required | Default | Bounds / reference |',
            '|---|---|---|---|---|']
    def cell(v):
        return json.dumps(v, ensure_ascii=False, separators=(',', ':')).replace('|','&#124;').replace('\n',' ')
    def walk(s, path):
        if not isinstance(s, dict): return
        for i, branch in enumerate(s.get('oneOf', s.get('anyOf', []))):
            tag = branch.get('properties',{}).get('type',{}).get('const',str(i+1))
            walk(branch, path+f'[{tag}]')
        for name, prop in s.get('properties', {}).items():
            field = path+'.'+name if path else name
            kind = prop.get('type', prop.get('const', prop.get('enum', 'union' if 'oneOf' in prop or 'anyOf' in prop else 'reference')))
            bounds = {k:v for k,v in prop.items() if k in ('minimum','maximum','minLength','maxLength','minItems','maxItems','uniqueItems','maxProperties','format','$ref','pattern')}
            rows.append(f'| `{field}` | {cell(kind)} | {"yes" if name in s.get("required",[]) else "no"} | {cell(prop["default"]) if "default" in prop else "none"} | {cell(bounds)} |')
            walk(prop,field)
            if 'items' in prop: walk(prop['items'],field+'[]')
        for name, definition in s.get('$defs',{}).items(): walk(definition,'$defs.'+name)
    walk(schema,'Product')
    return '\n'.join(rows)

def limits_table(contract):
    return '| Limit | Maximum |\n|---|---:|\n'+'\n'.join(f'| `{k}` | {v} |' for k,v in contract['limits'].items())
