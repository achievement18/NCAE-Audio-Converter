"""Build a deterministic compressed backend archive. No personal files are accepted."""
import argparse, json, struct, zlib
from pathlib import Path
parser = argparse.ArgumentParser()
parser.add_argument('backend', type=Path)
parser.add_argument('repo', type=Path)
parser.add_argument('output', type=Path)
args = parser.parse_args()
entries = []
for folder in ('runtime', 'converter'):
    for path in sorted((args.backend / folder).rglob('*')):
        if path.is_symlink() or path.is_junction():
            raise SystemExit(f'Linked runtime entry refused: {path.name}')
        if not path.is_file(): continue
        rel = path.relative_to(args.backend)
        if any(p in ('__pycache__', '.git', 'tests') for p in rel.parts) or path.suffix in ('.pyc', '.pyo'): continue
        if path.suffix.lower() in ('.ncae', '.bak', '.lnk') or path.name == 'replacement_records.json':
            raise SystemExit('Private content found in runtime')
        entries.append((rel.as_posix(), path.read_bytes()))
for name in ('LICENSE', 'THIRD_PARTY_NOTICES.md'):
    entries.append(('licenses/' + name, (args.repo / name).read_bytes()))
entries.append(('licenses/preview-cores.txt', (args.repo / 'apps/multiformat/reference/previews/LICENSES.txt').read_bytes()))
for path in sorted((args.repo / 'licenses').rglob('*')):
    if path.is_file(): entries.append(('licenses/' + path.relative_to(args.repo / 'licenses').as_posix(), path.read_bytes()))
for path in sorted((args.repo / 'docs').glob('*.md')):
    entries.append(('docs/' + path.name, path.read_bytes()))
for name in ('README.md', 'README.en.md'):
    entries.append(('docs/' + name, (args.repo / name).read_bytes()))
entries.sort()
if len({name.lower() for name, _ in entries}) != len(entries): raise SystemExit('Duplicate archive names')
raw = bytearray(b'NCAERUN1' + struct.pack('<I', len(entries)))
for name, data in entries:
    encoded = name.encode('utf-8')
    raw += struct.pack('<IQ', len(encoded), len(data)) + encoded + data
if len(raw) > 512 * 1024 * 1024: raise SystemExit('Runtime too large')
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_bytes(zlib.compress(raw, 9))
print(json.dumps(dict(files=len(entries), raw_bytes=len(raw), compressed_bytes=args.output.stat().st_size)))
