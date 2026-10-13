"""Convert an explicitly supplied SIX List One snapshot; never used at runtime."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import xml.etree.ElementTree as ET


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    raw = args.source.read_bytes()
    if len(raw) > 2 * 1024 * 1024 or b'<!DOCTYPE' in raw or b'<!ENTITY' in raw:
        raise ValueError('unexpected XML size or declaration')
    document = ET.fromstring(raw)
    publication = document.attrib['Pblshd']
    if not re.fullmatch(r'\d{4}-\d{2}-\d{2}', publication):
        raise ValueError('invalid publication date')
    table, excluded = {}, set()
    for row in document.findall('./CcyTbl/CcyNtry'):
        code = row.findtext('Ccy')
        unit = row.findtext('CcyMnrUnts')
        if code is None:
            continue
        if not re.fullmatch('[A-Z]{3}', code):
            raise ValueError('invalid currency code')
        if code in ('XTS', 'XXX') or unit == 'N.A.':
            excluded.add(code)
            continue
        if unit is None or not re.fullmatch('[0-9]', unit):
            raise ValueError(f'unexpected minor unit for {code}')
        value = int(unit)
        if code in table and table[code] != value:
            raise ValueError(f'inconsistent duplicate {code}')
        table[code] = value
    if not table or set(table) & excluded:
        raise ValueError('empty or conflicting table')
    data = (json.dumps(table, sort_keys=True, separators=(',', ':')) + '\n').encode('ascii')
    provenance = dict(schemaVersion='1.0', source='SIX ISO 4217 List One',
        sourceUrl='https://www.six-group.com/dam/download/financial-information/data-center/iso-currrency/lists/list-one.xml',
        publicationDate=publication, sourceSha256=hashlib.sha256(raw).hexdigest(),
        tableFile='minor-units.json', tableDigest='sha256:' + hashlib.sha256(data).hexdigest(),
        digestEncoding='SHA-256 of exact UTF-8 table bytes, including final LF',
        entryCount=len(table), excludedCodes=sorted(excluded),
        selection='Distinct current alphabetic codes with numeric minor units; exclude XTS, XXX and N.A.; no country names or standard text.')
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / 'minor-units.json').write_bytes(data)
    (args.out / 'provenance.json').write_bytes((json.dumps(provenance, indent=2) + '\n').encode('utf-8'))
    print(json.dumps(provenance, indent=2))


if __name__ == '__main__':
    main()
