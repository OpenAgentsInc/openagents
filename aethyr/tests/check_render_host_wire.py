#!/usr/bin/env python3
"""Actual browser scalar facade: canonical rawflags, untouched state, and loss controls."""
import argparse,hashlib,json,os,pathlib,signal,subprocess,time
ROOT=pathlib.Path(__file__).resolve().parents[2]
p=argparse.ArgumentParser();p.add_argument('--c',type=pathlib.Path,required=True);p.add_argument('--header',type=pathlib.Path,required=True);p.add_argument('--out',type=pathlib.Path,required=True);a=p.parse_args();out=a.out.resolve();out.mkdir(parents=True,exist_ok=True)
sha=lambda f:hashlib.sha256(pathlib.Path(f).read_bytes()).hexdigest()
inputs=[a.c.resolve(),a.header.resolve(),ROOT/'coax/web/render-face.c',ROOT/'aethyr/tests/render-host-wire.c',ROOT/'aethyr/render/host-policy.coax',ROOT/'aethyr/render/host.coax',pathlib.Path(__file__).resolve(),ROOT/'coax/bin/coax'];before={str(f):sha(f)for f in inputs};commands=[];r={'schema':'aethyr.render.browser-wire.v1','status':'running','inputs':before,'commands':commands}
def run(cmd,label,expect=0):
 start=time.monotonic()
 with (out/(label+'.log')).open('wb') as stream:
  child=subprocess.Popen(cmd,cwd=ROOT,stdout=stream,stderr=subprocess.STDOUT,start_new_session=True)
  try:code=child.wait(timeout=180)
  except BaseException:
   os.killpg(child.pid,signal.SIGTERM)
   try:child.wait(timeout=3)
   except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait(timeout=3)
   raise
 commands.append({'command':list(map(str,cmd)),'exit_code':code,'seconds':round(time.monotonic()-start,3),'log':str(out/(label+'.log'))})
 if expect==0:assert code==0,(label,code)
 else:assert code!=0,(label,'planted transport fault survived')
face=(ROOT/'coax/web/render-face.c').read_text();driver=(ROOT/'aethyr/tests/render-host-wire.c').read_text();header=a.header.read_bytes()
faults={
 'ack':('coax_t_RenderHostBool q=aethyr_render_bool(submitted);if(!q.ok)return;render_uploads.states.e[slot]=aethyr_render_upload_ack(render_uploads.states.e[slot],q.value);','render_uploads.states.e[slot]=aethyr_render_upload_ack(render_uploads.states.e[slot],submitted!=0);'),
 'pipeline':('coax_t_RenderHostBool q=aethyr_render_bool(ready);if(!q.ok)return COAX_RenderPackageStatus_invalid;return aethyr_render_package_pipeline(candidate_view(),entry,q.value);','return aethyr_render_package_pipeline(candidate_view(),entry,ready!=0);'),
 'mesh-capacity':('aethyr_render_mesh_placeholder(render_descriptor_profile,set,resource,device_capacity)','aethyr_render_mesh_placeholder(render_descriptor_profile,set,resource,16)'),
 'draw-object':('aethyr_render_words(address,render_draw_base,render_draw_capacity,bytes)','aethyr_render_words(address,0,memory_bytes,bytes)'),
 'draw-null':('coax_t_RenderHostByteView span=aethyr_render_asset_span(address,render_draw_capacity,bytes,memory_bytes);','coax_t_RenderHostByteView span={.status=COAX_RenderFrameStatus_ok};')}
try:
 for name,flags in [('debug',['-O0']),('optimized',['-O2']),('sanitized',['-O1','-fsanitize=address,undefined','-fno-omit-frame-pointer'])]:
  work=out/name;work.mkdir(exist_ok=True);(work/'Render.h').write_bytes(header);(work/'RenderFace.c').write_text(face);(work/'wire.c').write_text(driver)
  common=[os.environ.get('CC','cc'),'-std=c11','-ffp-contract=off','-fno-fast-math',*flags,'-I'+str(work),str(a.c.resolve()),str(work/'wire.c'),'-lm','-o']
  run(common+[str(work/'wire')],name+'-build');run([str(work/'wire')],name+'-run')
  for fault,(needle,replacement) in faults.items():
   assert face.count(needle)==1,(fault,'source control shape changed');(work/'RenderFace.c').write_text(face.replace(needle,replacement))
   run(common+[str(work/('fault-'+fault))],name+'-'+fault+'-build');run([str(work/('fault-'+fault))],name+'-'+fault+'-run',expect=1)
  (work/'RenderFace.c').write_text(face)
 r['source_changes']=[f for f,h in before.items()if sha(f)!=h];assert not r['source_changes'];r['status']='passed'
finally:(out/'qualification.json').write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps({'status':r['status'],'native_modes':3,'planted_controls_per_mode':len(faults),'source_changes':r.get('source_changes')}))
