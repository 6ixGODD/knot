"""Collect omitted upstream notices at each published crate's recorded Git commit."""
import hashlib
import json
from pathlib import Path
import subprocess
import urllib.parse
import urllib.request
import urllib.error

ROOT=Path(__file__).resolve().parents[2]
OUT=Path(__file__).resolve().parent/'notices'

def get(url):
    request=urllib.request.Request(url,headers={'User-Agent':'knot-notice-collector','Accept':'application/vnd.github+json'})
    with urllib.request.urlopen(request,timeout=45) as response: return response.read()

def main():
    metadata=json.loads(subprocess.check_output(['cargo','metadata','--format-version','1','--locked','--offline'],cwd=ROOT))
    OUT.mkdir(exist_ok=True)
    index_path=OUT/'index.json'
    index=json.loads(index_path.read_text(encoding='utf-8')) if index_path.exists() else {}
    groups={}
    current={}
    for package in metadata['packages']:
        if package['source'] is None: continue
        source=Path(package['manifest_path']).parent
        if any(p.is_file() and p.name.lower().startswith(('license','licence','copying','notice','copyright')) for p in source.rglob('*')): continue
        name=package['name']+'-'+package['version']
        if name in index: continue
        vcs=json.loads((source/'.cargo_vcs_info.json').read_text(encoding='utf-8'))
        revision=vcs['git']['sha1']
        previous=next((record for record in index.values() if record['repository']==package['repository'] and record['license']==package['license'] and not record.get('missingSeparateNotice')),None)
        if previous:
            notices=[dict(n,publishedRevisionMatched=n['revision']==revision) for n in previous['notices']]
            index[name]=dict(repository=package['repository'],revision=revision,pathInVcs=vcs.get('path_in_vcs',''),license=package['license'],notices=notices)
            index_path.write_text(json.dumps(index,indent=2)+'\n',encoding='utf-8')
            print(name+': reused pinned upstream notice snapshot',flush=True)
            continue
        repository=urllib.parse.urlparse(package['repository'])
        parts=repository.path.strip('/').removesuffix('.git').split('/')
        if repository.hostname=='git.ondrovo.com' and package['name']=='json_dotpath':
            # This upstream publishes only the Cargo license declaration, no separate notice.
            folder=OUT/name;folder.mkdir(exist_ok=True)
            path=folder/'UPSTREAM-Cargo.toml';path.write_bytes((source/'Cargo.toml.orig').read_bytes())
            index[name]=dict(repository=package['repository'],revision=revision,pathInVcs=vcs.get('path_in_vcs',''),license=package['license'],missingSeparateNotice=True,notices=[dict(path=path.relative_to(ROOT).as_posix(),url=package['repository'],revision=revision,publishedRevisionMatched=True,sha256=hashlib.sha256(path.read_bytes()).hexdigest())])
            index_path.write_text(json.dumps(index,indent=2)+'\n',encoding='utf-8')
            print(name+': retained upstream license declaration; separate notice absent',flush=True)
            continue
        if repository.scheme!='https' or repository.hostname!='github.com' or len(parts)!=2 or len(revision)!=40: raise RuntimeError('Unsupported official source '+str(package['repository']))
        repo='/'.join(parts)
        key=(repo,revision)
        if key not in groups:
            folder=OUT/(parts[0]+'-'+parts[1]+'-'+revision)
            folder.mkdir(exist_ok=True)
            notice_revision=revision
            try:
                tree=json.loads(get(f'https://api.github.com/repos/{repo}/git/trees/{revision}'))
            except urllib.error.HTTPError as error:
                if error.code==403:
                    tree={'tree':[{'type':'blob','path':filename} for filename in ['LICENSE','LICENSE-MIT','LICENSE-APACHE','LICENSE.md','LICENSE.txt','LICENCE.md','COPYING']]}
                elif error.code!=404: raise
                # Preserve provenance: never claim a current notice came from an unavailable commit.
                else:
                    if repo not in current:
                        current[repo]=subprocess.check_output(['git','ls-remote',f'https://github.com/{repo}.git','HEAD'],text=True).split()[0]
                    notice_revision=current[repo]
                    tree={'tree':[{'type':'blob','path':filename} for filename in ['LICENSE','LICENSE-MIT','LICENSE-APACHE','LICENSE.md','LICENSE.txt','LICENCE.md','COPYING']]}
                    print(f'{name}: published Git revision unavailable; notice snapshot at {notice_revision}',flush=True)
            notices=[]
            for entry in tree['tree']:
                if entry['type']!='blob' or not entry['path'].lower().startswith(('license','licence','copying','notice','copyright')): continue
                url=f'https://raw.githubusercontent.com/{repo}/{notice_revision}/{entry["path"]}'
                path=folder/entry['path']
                if not path.is_file():
                    try: path.write_bytes(get(url))
                    except urllib.error.HTTPError as error:
                        if error.code==404: continue
                        raise
                raw=path.read_bytes()
                if not raw.strip() or raw.lstrip().startswith(b'<!DOCTYPE'): raise RuntimeError('Invalid license body')
                notices.append(dict(path=path.relative_to(ROOT).as_posix(),url=url,sha256=hashlib.sha256(raw).hexdigest(),revision=notice_revision,publishedRevisionMatched=notice_revision==revision))
            if not notices:
                for filename in ['Cargo.toml.orig','README.md','AUTHORS']:
                    original=source/filename
                    if original.is_file():
                        path=folder/('UPSTREAM-'+filename);path.write_bytes(original.read_bytes())
                        notices.append(dict(path=path.relative_to(ROOT).as_posix(),url=package['repository'],revision=revision,publishedRevisionMatched=True,missingSeparateNotice=True,sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
                if not notices: raise RuntimeError('No license declaration at '+repo+' '+revision)
            groups[key]=notices
        index[name]=dict(repository=package['repository'],revision=revision,pathInVcs=vcs.get('path_in_vcs',''),license=package['license'],missingSeparateNotice=any(n.get('missingSeparateNotice',False) for n in groups[key]),notices=groups[key])
        index_path.write_text(json.dumps(index,indent=2)+'\n',encoding='utf-8')
        print(name,revision,flush=True)

if __name__=='__main__':main()
