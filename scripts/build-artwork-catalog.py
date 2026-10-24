"""Build a searchable preview catalog from an already-local IFS inventory."""
import argparse,hashlib,json
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor,as_completed
from ifstools import IFS
from PIL import Image,ImageDraw
p=argparse.ArgumentParser();p.add_argument('inventory');args=p.parse_args()
root=Path(args.inventory);inventory=json.loads((root/'inventory.json').read_text(encoding='utf8'));thumbs=root/'thumbnails';thumbs.mkdir(exist_ok=True)
def convert(a):
 entries=[];errors=[]
 if 'error' in a:return entries,errors
 try:
  i=IFS(str(root/'archives'/a['archive']),super_disable=True)
  try:
   images={f.name[:-4]:f for f in i.tree.all_files if type(f).__name__=='ImageFile'}
   for t in a['textures']:
    if t['width']<=0 or t['height']<=0:continue
    entry=dict(archive=a['archive'],texture=t['name'],width=t['width'],height=t['height'])
    f=images.get(t['name'])
    if f:
     name=hashlib.sha256((a['archive']+'\0'+t['name']).encode()).hexdigest()+'.png';dest=thumbs/name
     try:
      if not dest.exists():
       size,pixels=f.load(raw_pixels=True,crop_to_uvrect=True);im=Image.frombytes('RGBA',size,pixels);im.thumbnail((256,256));im.save(dest,compress_level=2)
      entry['preview']='thumbnails/'+name
     except Exception as e:errors.append(dict(archive=a['archive'],texture=t['name'],error=str(e)))
    entries.append(entry)
  finally:i.file.close()
 except Exception as e:errors.append(dict(archive=a['archive'],error=str(e)))
 return entries,errors
catalog=[];errors=[]
with ThreadPoolExecutor(max_workers=4) as pool:
 for index,result in enumerate(as_completed([pool.submit(convert,a) for a in inventory]),1):
  entries,failures=result.result();catalog.extend(entries);errors.extend(failures)
  if index%100==0:print(f'{index}/{len(inventory)} archives; {len(catalog)} entries; {len(errors)} preview errors',flush=True)
catalog.append(dict(archive='@movie',texture='main',width=1920,height=1080))
geometry_path=root/'geometry.json'
if geometry_path.exists():
 import math
 for archive in json.loads(geometry_path.read_text('utf8')):
  for shape in archive['shapes']:
   bounds=shape.get('bounds')
   if shape['flags']!=[9] or not bounds or not all(math.isfinite(v) for v in bounds):continue
   width=math.ceil(bounds[2]-bounds[0]);height=math.ceil(bounds[3]-bounds[1])
   if not (0<width<=16384 and 0<height<=16384):continue
   entry=dict(archive=archive['archive'],texture='@shape/'+shape['name'],width=width,height=height)
   if shape.get('points') and shape.get('triangles'):
    name='shape-'+shape['sha256']+'.png';dest=thumbs/name
    if not dest.exists():
     scale=min(256/width,256/height);im=Image.new('RGBA',(max(1,round(width*scale)),max(1,round(height*scale))))
     draw=ImageDraw.Draw(im);points=[((x-bounds[0])*scale,(y-bounds[1])*scale) for x,y in shape['points']]
     for indices,color in zip(shape['triangles'],shape['untextured_rgba']):
      for index in range(0,len(indices)-2,3):draw.polygon([points[i] for i in indices[index:index+3]],fill=tuple(bytes.fromhex(color)))
     im.save(dest,compress_level=2)
    entry['preview']='thumbnails/'+name
   catalog.append(entry)
catalog.sort(key=lambda e:(e['archive'],e['texture']))
(root/'catalog.json').write_text(json.dumps(catalog,ensure_ascii=False,separators=(',',':')),encoding='utf8')
(root/'preview-errors.json').write_text(json.dumps(errors,ensure_ascii=False,indent=2),encoding='utf8')
print(f'Done {len(catalog)} entries / {len(errors)} preview errors',flush=True)
