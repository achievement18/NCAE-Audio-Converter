"""Inspect the single EXE's direct and delayed PE imports without external dependencies."""
import argparse, json, struct
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('exe',type=Path);args=p.parse_args()
b=args.exe.read_bytes()
def u16(o):return struct.unpack_from('<H',b,o)[0]
def u32(o):return struct.unpack_from('<I',b,o)[0]
pe=u32(0x3c)
assert b[:2]==b'MZ' and b[pe:pe+4]==b'PE\0\0', 'Not a PE executable'
assert u16(pe+4)==0x8664, 'Expected Windows x64'
opt=pe+24
assert u16(opt)==0x20b, 'Expected PE32+'
sections=[]
for i in range(u16(pe+6)):
 o=opt+u16(pe+20)+40*i
 sections.append((u32(o+12),max(u32(o+8),u32(o+16)),u32(o+20)))
def offset(rva):
 for start,size,raw in sections:
  if start<=rva<start+size:return raw+rva-start
 raise ValueError('Unmapped PE address')
def name(rva):
 o=offset(rva);return b[o:b.index(0,o)].decode('ascii')
imports=[]
for index,stride,name_offset in ((1,20,12),(13,32,4)):
 rva=u32(opt+112+index*8)
 if not rva:continue
 o=offset(rva)
 while any(b[o:o+stride]):
  if index==13 and not u32(o)&1:raise ValueError('Unsupported VA-based delay import')
  imports.append(name(u32(o+name_offset)));o+=stride
bad=[v for v in imports if v.lower().startswith(('vcruntime','msvcp','api-ms-win-crt')) or v.lower()=='ucrtbase.dll']
if bad:raise SystemExit('External Visual C++ runtime dependencies: '+', '.join(bad))
if len(b)<20*1024*1024:raise SystemExit('EXE is unexpectedly small; embedded runtime may be missing')
print(json.dumps(dict(single_exe=True,bytes=len(b),imports=sorted(set(imports))),indent=2))
