"""Collect dependency notices without leaking registry paths into the repository."""
import json, subprocess
from pathlib import Path
repo=Path(__file__).resolve().parents[1]
command=['cargo','metadata','--locked','--offline','--format-version','1','--filter-platform','x86_64-pc-windows-msvc','--manifest-path',str(repo/'apps/multiformat/Cargo.toml')]
metadata=json.loads(subprocess.check_output(command))
root=repo/'licenses/rust';root.mkdir(parents=True,exist_ok=True)
previous_path=root/'DEPENDENCIES.json'
previous=json.loads(previous_path.read_text(encoding='utf-8')) if previous_path.exists() else []
records=[]
for package in sorted(metadata['packages'],key=lambda p:(p['name'],p['version'])):
    if package['source'] is None:continue
    base=Path(package['manifest_path']).parent
    paths=[]
    for path in base.iterdir():
        if path.is_file() and path.name.upper().startswith(('LICENSE','LICENCE','COPYING','NOTICE')):paths.append(path)
        elif path.is_dir() and path.name.lower() in ('licenses','license'):
            paths.extend(p for p in path.rglob('*') if p.is_file())
    if package.get('license_file'):
        named=base/package['license_file']
        if named.is_file():paths.append(named)
    saved=[]
    for path in sorted(set(paths)):
        relative=path.relative_to(base)
        target=root/(package['name']+'-'+package['version'])/relative
        target.parent.mkdir(parents=True,exist_ok=True)
        target.write_bytes(path.read_bytes())
        saved.append(target.relative_to(root).as_posix())
    destination=root/(package['name']+'-'+package['version'])
    if destination.is_dir(): saved=sorted(p.relative_to(root).as_posix() for p in destination.rglob('*') if p.is_file())
    record=dict(name=package['name'],version=package['version'],license=package.get('license'),repository=package.get('repository'),notices=saved)
    old=next((x for x in previous if x['name']==package['name'] and x['version']==package['version']),{})
    if 'notice_source' in old:record['notice_source']=old['notice_source']
    records.append(record)
(root/'DEPENDENCIES.json').write_text(json.dumps(records,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps(dict(dependencies=len(records),notice_files=sum(len(x['notices']) for x in records),without_bundled_notice=[x['name'] for x in records if not x['notices']])))
