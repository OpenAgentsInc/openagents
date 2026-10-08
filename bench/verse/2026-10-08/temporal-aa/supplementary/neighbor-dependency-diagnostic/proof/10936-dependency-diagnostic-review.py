from pathlib import Path
from PIL import Image,ImageDraw
import hashlib,json
p=Path(__file__).resolve().parent
new=p/'10936-dependency-high-diagnostic'
old=p/'10936-high-reactive-release-paired'
out=p/'10936-dependency-diagnostic-review';out.mkdir(exist_ok=True)
receipt=json.loads((p/'10936-dependency-diagnostic-gpu.json').read_text())
assert receipt['exit']==0 and receipt.get('released_at_ms')
records={}
box=(490,250,820,565)
for frames in [(470,471,472),(473,474)]:
 sheet=Image.new('RGB',(4*(box[2]-box[0]),len(frames)*(box[3]-box[1]+24)),(15,15,15))
 draw=ImageDraw.Draw(sheet)
 for row,frame in enumerate(frames):
  for col,(prefix,root,aa) in enumerate([('old off',old,'off'),('old on',old,'on'),('new off',new,'off'),('new on',new,'on')]):
   f=root/'frames'/f'{frame:04}-taa-{aa}.png'
   image=Image.open(f).convert('RGB'); assert image.size==(1920,1080)
   key=f'{prefix}/{frame}'
   records[key]={'source':str(f),'bytes':f.stat().st_size,'sha256':hashlib.sha256(f.read_bytes()).hexdigest(),'crop':list(box)}
   crop=image.crop(box);crop.save(out/f'{prefix.replace(" ","-")}-{frame}.png')
   xy=(col*(box[2]-box[0]),row*(box[3]-box[1]+24))
   draw.text(xy,f'{prefix} frame {frame}',fill='white')
   sheet.paste(crop,(xy[0],xy[1]+24))
 sheet.save(out/f'head-{frames[0]}-{frames[-1]}.png')
(out/'sources.json').write_text(json.dumps({'source_old':'36df9ceb4bbb7fb0c7be30bd1c3538e0be78ab3e','source_new':'290c1590726189dbdabb5693125cfa0fbca8a779','new_profile':'dev diagnostic, no timing gate','images':records},indent=2)+'\n')
print(out)
