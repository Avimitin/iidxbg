"""Audit local GE2D shapes to distinguish texture-backed and vector-only assets.
Header layout cross-checked against bemaniutils/bemani/format/afp/geo.py.
"""
import argparse,collections,hashlib,json,struct
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor,as_completed
from ifstools import IFS
p=argparse.ArgumentParser();p.add_argument('inventory');args=p.parse_args();root=Path(args.inventory)
inventory=json.loads((root/'inventory.json').read_text('utf-8'))
def scan(a):
 out=dict(archive=a['archive'],sha256=a['sha256'],shapes=[],errors=[])
 if 'error' in a:return out
 i=IFS(str(root/'archives'/a['archive']),super_disable=True)
 try:
  for f in i.tree.all_files:
   if not f.full_path.replace('\\','/').startswith('geo/'):continue
   try:
    b=f.load();endian='<' if b[:4]==b'D2EG' else '>' if b[:4]==b'GE2D' else None
    if not endian:raise ValueError('unknown geometry magic '+b[:8].hex())
    size,flags=struct.unpack_from(endian+'II',b,12)
    if size!=len(b):raise ValueError('geometry length mismatch')
    vc,tc,cc,lc,rc,_=struct.unpack_from(endian+'6H',b,20);vo,to,co,lo,ro=struct.unpack_from(endian+'5I',b,32)
    labels=[]
    for n in range(lc):
     offset=struct.unpack_from(endian+'I',b,lo+n*4)[0];v=b[offset:b.index(0,offset)]
     if v and v[0]>=160:v=bytes((c+128)&255 for c in v)
     labels.append(v.decode('ascii'))
    draws=[struct.unpack_from(endian+'BBBBHHII',b,ro+n*16) for n in range(rc)]
    regions=[labels[d[2]] for d in draws if d[1]&2]
    points=[struct.unpack_from(endian+'ff',b,vo+n*8) for n in range(vc)]
    bounds=[min(p[0] for p in points),min(p[1] for p in points),max(p[0] for p in points),max(p[1] for p in points)] if points else None
    colors=[b[ro+n*16+8:ro+n*16+12].hex() for n,d in enumerate(draws) if not(d[1]&2)]
    shape=dict(name=f.name,sha256=hashlib.sha256(b).hexdigest(),vertices=vc,draws=rc,regions=regions,untextured=sum(not(d[1]&2) for d in draws),flags=sorted(set(d[1] for d in draws)),bounds=bounds,untextured_rgba=colors)
    if shape['untextured']:
     shape['points']=points
     shape['triangles']=[list(struct.unpack_from(endian+str(d[4])+'H',b,d[7])) for d in draws if not(d[1]&2)]
    out['shapes'].append(shape)
   except Exception as e:out['errors'].append(dict(name=f.name,error=str(e)))
 finally:i.file.close()
 return out
results=[]
with ThreadPoolExecutor(max_workers=4) as pool:
 for future in as_completed([pool.submit(scan,a) for a in inventory]):
  results.append(future.result())
  if len(results)%500==0:print(f'{len(results)}/{len(inventory)}',flush=True)
results.sort(key=lambda a:a['archive']);(root/'geometry.json').write_text(json.dumps(results,ensure_ascii=False,separators=(',',':')),'utf-8')
print(dict(shapes=sum(len(a['shapes']) for a in results),untextured=sum(s['untextured']>0 for a in results for s in a['shapes']),errors=sum(len(a['errors']) for a in results)),flush=True)
