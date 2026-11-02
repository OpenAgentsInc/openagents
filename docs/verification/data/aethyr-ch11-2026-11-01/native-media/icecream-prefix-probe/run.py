from pathlib import Path
import hashlib,json,shutil,subprocess,tarfile,time,struct
root=Path('/Users/christopherdavid/work/openagents-aethyr-book');old=Path('/tmp/aethyr-ch11-native-icecream');work=Path('/tmp/aethyr-ch11-native-icecream-prefixes');stage=work/'stage';work.mkdir(exist_ok=True)
if stage.exists():raise SystemExit('fresh stage required')
shutil.copytree(old/'stage',stage)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();original={str(p.relative_to(stage)):sha(p)for p in stage.rglob('*')if p.is_file()};(work/'original-stage.json').write_text(json.dumps(original,indent=2)+'\n')
host=stage/'render-host.c';s=host.read_text();s=s.replace('typedef struct {VksPipe pipe;','int rhg_diag_target=COAX_RenderTarget_screen;\ntypedef struct {VksPipe pipe;',1);s=s.replace('coax_t_RenderHostClear clear=aethyr_render_clear();','coax_t_RenderHostClear clear=aethyr_render_clear();\n  const char *cut=getenv("RHG_STOP");int stop=cut?atoi(cut):-1;coax_t_RenderTarget active_target=COAX_RenderTarget_screen;',1);s=s.replace('coax_t_RenderOp op=plan->ops.e[i];\n    switch(op.kind){','coax_t_RenderOp op=plan->ops.e[i];\n    if(op.kind==COAX_RenderOpKind_begin)active_target=op.colour0;\n    switch(op.kind){',1)
needle='    }\n  }\n}\nint rhg_execute';assert needle in s;s=s.replace(needle,'    }\n    if(op.kind==COAX_RenderOpKind_draw&&(int)op.pass==stop){rhg_diag_target=active_target;vks_surface_end(cmd);return;}\n  }\n}\nint rhg_execute',1)
# Effects-only preserve the depth attachment for diagnostic readback. Colour and
# all shader/geometry/depth-test arithmetic remain unchanged; full identity gates it.
s=s.replace('vks_surface_open(r->v,&r->surfaces[i],&attachments,op.store_depth);','vks_surface_open(r->v,&r->surfaces[i],&attachments,1);');host.write_text(s)
p=stage/'native-frame.c';s=p.read_text().replace('typedef struct OwnedBuffer','extern int rhg_diag_target;\ntypedef struct OwnedBuffer',1);s=s.replace('VksImage *screen=rhg_screen(frame.renderer);','VksImage *screen=rhg_target_image(frame.renderer,(coax_t_RenderTarget)rhg_diag_target);');s=s.replace('c[0],COAX_RenderTarget_screen,screen->w','c[0],rhg_diag_target,screen->w')
needle='  FILE *record=fopen(argv[4],"w");';assert needle in s
s=s.replace(needle,'  const char *depth_path=getenv("RHG_DEPTH");if(depth_path){VksImage *depth=rhg_target_image(frame.renderer,COAX_RenderTarget_scene_depth);if(depth){size_t depth_bytes=(size_t)depth->w*depth->h*4;void *dp=malloc(depth_bytes);assert(dp);vks_read(&frame.device,depth,dp,depth_bytes);FILE *df=fopen(depth_path,"wb");assert(df&&fwrite(dp,1,depth_bytes,df)==depth_bytes&&!fclose(df));free(dp);}}\n'+needle);p.write_text(s)
(stage/'remote.sh').write_text('''set -euo pipefail
export VK_DRIVER_FILES=/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json
timeout 600 nix-shell -p gcc vulkan-headers vulkan-loader vulkan-validation-layers --run '
set -e
gcc -std=c11 -O2 -ffunction-sections -fdata-sections -Wl,--gc-sections -ffp-contract=off -fno-fast-math -Wno-unknown-pragmas -I. native-frame.c render-host.c garden.c Grid.c Player.c RenderTexture.c vks.c vkc.c -lvulkan -lm -o native-frame >compile.log 2>&1
export VK_ADD_LAYER_PATH=$(dirname $(find /nix/store -maxdepth 6 -name VkLayer_khronos_validation.json -path "*validation-layers*" | head -1))
export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation
for case in full:-1 bg:0 scene:5 hud:12;do name=${case%:*};cut=${case#*:};RHG_STOP=$cut RHG_DEPTH=$name.depth32 ./native-frame frame.config package/render.json $name.raw $name.json rgba16f >$name.log 2>&1;done
'
''')
with tarfile.open(work/'send.tar','w')as t:
 for p in stage.rglob('*'):
  if p.is_file()and p.name!='native-frame':t.add(p,arcname=str(p.relative_to(stage)))
remote="set -e;d=$(mktemp -d);trap 'rm -rf \"$d\"' EXIT;tar -xf - -C \"$d\";cd \"$d\";status=0;bash remote.sh >remote.log 2>&1||status=$?;echo $status >status.txt;tar -cf - --ignore-failed-read status.txt remote.log compile.log native-frame full.raw full.depth32 full.json full.log bg.raw bg.depth32 bg.json bg.log scene.raw scene.depth32 scene.json scene.log hud.raw hud.depth32 hud.json hud.log 2>/dev/null"
start=time.monotonic()
with (work/'send.tar').open('rb')as inp,(work/'returned.tar').open('wb')as out,(work/'ssh.log').open('wb')as err:r=subprocess.run(['ssh','-o','BatchMode=yes','-o','ConnectTimeout=15','coderos-4080',remote],stdin=inp,stdout=out,stderr=err,timeout=700)
assert r.returncode==0
allowed={'status.txt','remote.log','compile.log','native-frame'}|{f'{n}.{s}'for n in ['full','bg','scene','hud']for s in ['raw','depth32','json','log']}
with tarfile.open(work/'returned.tar')as t:
 for m in t.getmembers():
  assert m.name in allowed and m.isfile();(work/m.name).write_bytes(t.extractfile(m).read())
status=(work/'status.txt').read_text().strip();assert status=='0',(status,(work/'remote.log').read_text()[-2000:])
assert (work/'full.raw').read_bytes()==(old/'screen.raw').read_bytes(),'instrumented full colour image changed'
pixels=[(x,98)for x in range(287,294)]+[(x,y)for y in range(99,102)for x in range(286,294)]+[(x,103)for x in range(285,293)];rows=[]
for name in ['bg','scene','hud','full']:
 q=json.loads((work/f'{name}.json').read_text());assert q['validation_errors']==q['validation_warnings']==0;raw=(work/f'{name}.raw').read_bytes();depth=(work/f'{name}.depth32').read_bytes()
 rows.append({'stage':name,'frame':q,'pixels':[{'x':x,'y':y,'rgba':list(struct.unpack_from('<4e',raw,(y*q['width']+x)*8)),'depth':struct.unpack_from('<f',depth,(y*q['width']+x)*4)[0]if name!='bg'else None}for x,y in pixels]})
q={'schema':'aethyr.render.native-causal-prefix.v1','status':'pass','scope':'Effects-only diagnostic prefixes of exact retained IceCream producer input/products and historical source epoch. Depth store enabled for observation; instrumented full colour image is byte-identical to retained baseline. Prefixes are causal diagnostics, not ordinary product frames. BG depth is uninitialized and not interpreted.','elapsed_seconds':time.monotonic()-start,'full_identity_sha256':sha(work/'full.raw'),'expected_full_sha256':sha(old/'screen.raw'),'original_stage':original,'instrumented_files':{str(p.relative_to(stage)):sha(p)for p in [stage/'render-host.c',stage/'native-frame.c',stage/'remote.sh']},'observations':rows};(work/'pixel-stages.json').write_text(json.dumps(q,indent=2)+'\n');print(json.dumps({'status':q['status'],'full_identity_sha256':q['full_identity_sha256'],'stages':len(rows),'pixels_per_stage':len(pixels)}))
