from pathlib import Path
import hashlib,json,shutil
import numpy as np
from PIL import Image,ImageDraw
scratch=Path('/Users/christopherdavid/.openagents/scratch/codex-01a119ae-8a0b-7850-9b7f-ca9fe5a3203b')
root=Path('/Users/christopherdavid/.codex/worktrees/bb65/openagents/bench/verse/2026-10-08/temporal-aa')
crop=(490,250,820,565)
def read(p):return json.loads(p.read_text())
def write(p,v):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(v,indent=2)+'\n')
def ident(p):return {'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size}
for prefix,folder,title in [
 ('10936-stage-diagnostics','stage-history-1d38','Resolved current, new history, and sharpened scene'),
 ('10936-current-footprint','current-footprint-bfa','Exact current color at the negative footprint')]:
 source=scratch/prefix;out=root/'supplementary'/folder
 manifest=read(scratch/(prefix+'-manifest.json'));lease=read(scratch/(prefix+'-gpu.json'));report=read(source/'capture.json')
 assert manifest['exit']==lease['exit']==0 and lease['held_whole_run'] and lease['released_at_ms']>=manifest['end_unix']*1000
 records=report['temporal_texture_diagnostics']['frames'];assert [r['frame'] for r in records]==list(range(464,477))
 out.mkdir(parents=True,exist_ok=True)
 shutil.copy2(source/'capture.json',out/'capture.json')
 for proof in sorted(scratch.glob(prefix+'*')):
  if proof.is_file() and proof.suffix not in ['.png']:
   (out/'proof').mkdir(exist_ok=True);shutil.copy2(proof,out/'proof'/proof.name)
 shutil.copy2(scratch/'10936-stage-evidence-curate.py',out/'proof/curate-attempts.py')
 inventory={}
 for path in sorted(source.rglob('*.png')):
  relative=path.relative_to(source)
  with Image.open(path) as image:pixels=list(image.size)
  row={'path':str(path),**ident(path),'pixels':pixels,'storage':'durable_scratch'}
  target=None
  if path.name.endswith('-marker.png') or path.name.endswith('-history-reactive.png'):target=out/'masks'/path.name
  elif str(relative) in ['frames/0471-taa-on.png','frames/0471-taa-off.png']:target=out/'selected'/path.name
  if target:
   target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(path,target);row.update(storage='git',file=str(target.relative_to(out)))
  inventory[str(relative)]=row
 observations=[]
 for frame in range(469,473):
  diag=source/'temporal-diagnostics';base=np.array(Image.open(diag/f'{frame:04}-hdr-before-temporal.png').convert('RGB'));hist=np.array(Image.open(diag/f'{frame:04}-hdr-history.png').convert('RGB'));negative=np.array(Image.open(diag/f'{frame:04}-history-reactive.png').convert('L'))>0
  delta=np.abs(base.astype(np.int16)-hist.astype(np.int16));chosen=delta[negative]
  roi=(558,290,620,345) if frame==471 else (580,338,645,385) if frame==472 else None
  row={'frame':frame,'negative_pixels':int(negative.sum()),'negative_footprint_current_history_changed_pixels':int(np.any(delta>0,axis=2)[negative].sum()),'negative_footprint_max_display_byte_difference':int(chosen.max())}
  if roi:
   x0,y0,x1,y1=roi;row['ghost_roi']=list(roi);row['ghost_roi_current_negative_pixels']=int(negative[y0:y1,x0:x1].sum());row['ghost_roi_current_history_changed_pixels']=int(np.any(delta[y0:y1,x0:x1]>0,axis=2).sum());row['ghost_roi_max_display_byte_difference']=int(delta[y0:y1,x0:x1].max())
  observations.append(row)
  inputs=[source/'frames'/f'{frame:04}-taa-off.png',source/'frames'/f'{frame:04}-taa-on.png',diag/f'{frame:04}-hdr-before-temporal.png',diag/f'{frame:04}-hdr-history.png',diag/f'{frame:04}-hdr-scene.png',diag/f'{frame:04}-marker.png',diag/f'{frame:04}-history-reactive.png']
  labels=['AA off','AA on','Current pre-TAA','New history','After sharpening','Body R8','Negative history']
  w,h=crop[2]-crop[0],crop[3]-crop[1];sheet=Image.new('RGB',(w*len(inputs),h+25))
  for k,(path,label) in enumerate(zip(inputs,labels)):
   with Image.open(path) as image:native=image.convert('RGB').crop(crop)
   target=out/'audit'/f'{frame}-{label.lower().replace(" ","-")}.png';target.parent.mkdir(exist_ok=True);native.save(target)
   sheet.paste(native,(k*w,25));ImageDraw.Draw(sheet).text((k*w+4,5),f'{frame} {label}',fill='white')
  sheet.save(out/'audit'/f'{frame}-stages.png')
 write(out/'audit/observations.json',{'source':manifest['source'],'crop':list(crop),'method':'Exact PNG byte comparisons after the same bounded HDR display transform. Quantized comparisons cannot recover original float differences. Raw source pixel positions do not establish reprojected age.','frames':observations,'visual':'Detached contour remains in AA-on frames 471 and 472, absent from AA-off/current pre-TAA. New history contains the contour before sharpening; no visual acceptance.'})
 write(out/'source-images.json',{'schema':'openagents.verse.image-inventory.v1','images':inventory})
 write(out/'verification.json',{'schema':'openagents.verse.temporal-visual-attempt.v1','status':'visual_failed','source':manifest['source'],'binary_sha256':manifest['binary_sha256'],'profile':manifest['profile'],'features':manifest['features'],'timing_acceptance_available':False,'supersedes_top_level_acceptance':False,'simulation_frames':540,'sequence_frames':[464,476],'command_manifest':'proof/'+prefix+'-manifest.json','gpu_receipt':'proof/'+prefix+'-gpu.json','report':'capture.json','source_images':'source-images.json','observations':'audit/observations.json','limitations':['GPU-only run overlaps CPU work; additional snapshot/readbacks exclude timing acceptance','Display-normalized PNGs are quantized; no raw float equality or exact temporal duration claim','All original PNG identities remain in the inventory; selected full originals and native crops remain in Git']})
 (out/'README.md').write_text(f'{title} on source `{manifest["source"]}`, binary SHA-256 `{manifest["binary_sha256"]}`. This High 9-second diagnostic remains a visual failure: detached contours persist at frames 471 and 472. No timing gate or acceptance promotion is claimed.\n\n[Stage comparisons](audit/471-stages.png) place AA off, AA on, exact pre-TAA snapshot, new history, sharpened scene, body marker, and negative-history mask side by side at native crop `{list(crop)}`. [Observations](audit/observations.json) retain quantized current/history differences and their limits. [capture.json](capture.json) retains every frame, diagnostic record, matrix, timing sample, and outlier; [source-images.json](source-images.json) retains SHA-256, size, and original location for all {len(inventory)} PNGs. All masks and the full frame 471 pair remain in Git; other originals remain in durable scratch.\n')
 print(folder,manifest['source'],len(inventory),'images',json.dumps(observations))
