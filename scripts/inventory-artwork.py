"""Inventory IFS metadata from local copies only; never extract on the game share."""
import argparse,hashlib,json,shutil
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor,as_completed
from ifstools import IFS
from kbinxml import KBinXML
p=argparse.ArgumentParser();p.add_argument('source');p.add_argument('output');args=p.parse_args()
source=Path(args.source).resolve();out=Path(args.output).resolve()
if str(out).startswith('\\\\') or out==source or source in out.parents:
 raise SystemExit('Output must be a local analysis directory outside the game source.')
out.mkdir(parents=True,exist_ok=True)
def digest_file(path):
 h=hashlib.sha256()
 with path.open('rb') as stream:
  for block in iter(lambda:stream.read(1024*1024),b''):h.update(block)
 return h.hexdigest()
archives=out/'archives';archives.mkdir(exist_ok=True)
def scan(path):
 rel=path.relative_to(source);local=archives/rel;local.parent.mkdir(parents=True,exist_ok=True)
 source_digest=digest_file(path)
 if not local.exists() or digest_file(local)!=source_digest:shutil.copy2(path,local)
 digest=digest_file(local)
 if digest!=source_digest:raise RuntimeError(f'Source changed during copy: {rel}')
 result=dict(archive=rel.as_posix(),sha256=digest,bytes=local.stat().st_size,textures=[],files=[])
 try:
  i=IFS(str(local),super_disable=True)
  try:
   for f in i.tree.all_files:
    result['files'].append(dict(name=f.full_path.replace('\\','/'),kind=type(f).__name__,size=getattr(f,'size',None)))
    if f.name=='texturelist.xml':
     k=KBinXML(f.load())
     for atlas in k.xml_doc.findall('texture'):
      size=[int(x) for x in atlas.findtext('size').split()]
      for image in atlas.findall('image'):
       uv=[int(x)/2 for x in image.findtext('uvrect').split()]
       result['textures'].append(dict(name=image.get('name'),atlas=atlas.get('name'),format=atlas.get('format'),atlas_size=size,uvrect=uv,width=int(uv[1]-uv[0]),height=int(uv[3]-uv[2])))
  finally:i.file.close()
 except Exception as e:result['error']=str(e)
 return result
paths=list(source.rglob('*.ifs'));results=[]
with ThreadPoolExecutor(max_workers=3) as pool:
 for future in as_completed([pool.submit(scan,p) for p in paths]):
  r=future.result();results.append(r)
  if len(results)%100==0:print(f'{len(results)}/{len(paths)} archives, {sum(len(x["textures"]) for x in results)} textures',flush=True)
  (out/'inventory.partial.json').write_text(json.dumps(results,ensure_ascii=False),encoding='utf8') if len(results)%500==0 else None
results.sort(key=lambda x:x['archive'])
(out/'inventory.json').write_text(json.dumps(results,ensure_ascii=False,indent=2),encoding='utf8')
print(f'Done: {len(results)} archives, {sum(len(x["textures"]) for x in results)} textures, {sum("error" in x for x in results)} errors',flush=True)
