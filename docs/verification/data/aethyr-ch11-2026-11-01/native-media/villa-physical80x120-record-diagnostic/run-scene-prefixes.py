from pathlib import Path
import subprocess,tarfile,json,hashlib,shutil,time
import argparse
p=argparse.ArgumentParser();p.add_argument('slug');p.add_argument('config',type=Path);p.add_argument('work',type=Path);p.add_argument('--reuse-executable',type=Path);p.add_argument('--package',type=Path);p.add_argument('--skip-spv-validation',action='store_true');a=p.parse_args()
root=Path('/Users/christopherdavid/work/openagents-aethyr-book');base=Path('/tmp/aethyr-ch11-native-render');work=a.work;work.mkdir(exist_ok=True);stage=work/'stage';stage.mkdir(exist_ok=True);(stage/'inputs').mkdir(exist_ok=True);(stage/'package').mkdir(exist_ok=True)
config=a.config;package=a.package or Path('/tmp/aethyr-ch11-packages-final-v2')/a.slug/'render.json';tokens=config.read_text().split();inputs={};staged={}
def copy(p):
 if p=='-':return '-'
 path=Path(p);path=path if path.is_absolute()else config.parent/path
 target='inputs/'+path.name;data=path.read_bytes();digest=hashlib.sha256(data).hexdigest()
 if target in staged:assert staged[target]==digest
 else:(stage/target).write_bytes(data);staged[target]=digest
 inputs[str(path)]=digest;return target
for i in range(13,18):tokens[i]=copy(tokens[i])
tokens[17]='package/render.json';at=18;n=int(tokens[at]);at+=1
for i in range(n):tokens[at+1]=copy(tokens[at+1]);at+=2
n=int(tokens[at]);at+=1
for i in range(n):tokens[at+1]=copy(tokens[at+1]);at+=8
if at<len(tokens)and tokens[at]=='textures':
 n=int(tokens[at+1]);at+=2
 for i in range(n):
  levels=int(tokens[at+3]);tokens[at+4]=copy(tokens[at+4]);at+=5+levels*5
if at<len(tokens)and tokens[at]=='media_text':tokens[at+1]=copy(tokens[at+1]);at+=2
if at<len(tokens)and tokens[at]=='media_images':at+=2+5*int(tokens[at+1])
assert at==len(tokens),'unexpected input suffix' 
(stage/'frame.config').write_text('\n'.join(tokens)+'\n')
for p in package.parent.iterdir():
 if p.is_file()and(p.name=='render.json'or p.suffix=='.spv'):(stage/'package'/p.name).write_bytes(p.read_bytes());inputs[str(p)]=hashlib.sha256(p.read_bytes()).hexdigest()
sources=[root/'verse/native'/n for n in ['render-host.c','render-host.h','render-textures.c','render-textures.h','render-media.c','render-media.h']]+[root/'aethyr/render/tests'/n for n in ['native-frame.c','frame-input.h']]+[root/'verse/gpu'/n for n in ['vks.c','vks.h','vkc.c','vkc.h']]+[Path('/tmp/aethyr-ch11-native-mesh-placeholder')/n for n in ['RenderHost.c','RenderHost.h']]+[Path('/tmp/aethyr-ch11-native-build-garden-allocation/render-native')/n for n in ['RenderTexture.c','RenderTexture.h']]
for p in sources:(stage/p.name).write_bytes(p.read_bytes());inputs[str(p)]=hashlib.sha256(p.read_bytes()).hexdigest()
# Effects-only individual-scene-record prefixes and raw attachment readback.
host=stage/'render-host.c';code=host.read_text();code=code.replace('typedef struct {VksPipe pipe;', 'int rhg_diag_target=COAX_RenderTarget_screen;\ntypedef struct {VksPipe pipe;',1)
code=code.replace('coax_t_RenderHostClear clear=aethyr_render_clear();','coax_t_RenderHostClear clear=aethyr_render_clear();\n  const char *cut=getenv("RHG_STOP");int stop=cut?atoi(cut):-1;',1)
pattern='else for(int64_t k=0;k<op.count;k++)rhg_record_draw(r,cmd,r->decoded[op.first+k]);'
replacement='else for(int64_t k=0;k<op.count;k++){rhg_record_draw(r,cmd,r->decoded[op.first+k]);if((int)op.pass==5&&k==stop){rhg_diag_target=COAX_RenderTarget_light;vks_surface_end(cmd);return;}}'
assert pattern in code;code=code.replace(pattern,replacement,1);host.write_text(code)
driver=stage/'native-frame.c';code=driver.read_text().replace('typedef struct OwnedBuffer','extern int rhg_diag_target;\ntypedef struct OwnedBuffer',1).replace('VksImage *screen=rhg_screen(frame.renderer);','VksImage *screen=rhg_target_image(frame.renderer,(coax_t_RenderTarget)rhg_diag_target);').replace('c[0],COAX_RenderTarget_screen,screen->w','c[0],rhg_diag_target,screen->w')
needle='  FILE *record=fopen(argv[4],"w");'
extra='  for(int target=2;target<=4;target++){VksImage *image=rhg_target_image(frame.renderer,(coax_t_RenderTarget)target);assert(image);size_t size=(size_t)image->w*image->h*(target==4?4:8);void *data=malloc(size);assert(data);vks_read(&frame.device,image,data,size);char name[512];assert(snprintf(name,sizeof name,"%s.target%d",argv[3],target)>0);FILE *file=fopen(name,"wb");assert(file&&fwrite(data,1,size,file)==size&&!fclose(file));free(data);}\n'
assert needle in code;code=code.replace(needle,extra+needle,1);driver.write_text(code)
if a.reuse_executable:
 previous=Path('/tmp/aethyr-ch11-native-media-actual-villa/stage')
 for source in sources:
  assert source.read_bytes()==(previous/source.name).read_bytes(),f'changed executable source: {source}'
 (stage/'native-frame').write_bytes(a.reuse_executable.read_bytes());inputs[str(a.reuse_executable)]=hashlib.sha256(a.reuse_executable.read_bytes()).hexdigest()
(stage/'remote.sh').write_text('''set -euo pipefail
export VK_DRIVER_FILES=/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json
timeout 600 nix-shell -p gcc vulkan-headers vulkan-loader vulkan-validation-layers spirv-tools --run '
set -e
for p in package/*.spv;do spirv-val --target-env vulkan1.1 "$p";done
gcc -std=c11 -O2 -ffunction-sections -fdata-sections -Wl,--gc-sections -ffp-contract=off -fno-fast-math -Wno-unknown-pragmas -I. native-frame.c render-host.c render-textures.c render-media.c RenderHost.c RenderTexture.c vks.c vkc.c -lvulkan -lm -o native-frame >compile.log 2>&1
export VK_ADD_LAYER_PATH=$(dirname $(find /nix/store -maxdepth 6 -name VkLayer_khronos_validation.json -path "*validation-layers*" | head -1))
export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation
for case in full:-1 draw0:0 draw1:1 draw2:2 draw3:3 draw4:4 draw5:5;do name=${case%:*};cut=${case#*:};RHG_STOP=$cut ./native-frame frame.config package/render.json $name.raw $name.json rgba16f >$name.log 2>&1;done
cp full.raw screen.raw;cp full.json frame.json;cp full.log native.log
'
''')
if a.reuse_executable:
 remote_text=(stage/'remote.sh').read_text();remote_text='\n'.join('chmod +x native-frame'if line.startswith('gcc ')else line for line in remote_text.splitlines())+'\n';(stage/'remote.sh').write_text(remote_text)
if a.skip_spv_validation:
 text=(stage/'remote.sh').read_text();text='\n'.join(line for line in text.splitlines()if not line.startswith('for p in package/'))+'\n';(stage/'remote.sh').write_text(text)
with tarfile.open(work/'send.tar','w')as archive:
 for p in stage.rglob('*'):
  if p.is_file():archive.add(p,arcname=str(p.relative_to(stage)))
remote="set -e;d=$(mktemp -d);trap 'rm -rf \"$d\"' EXIT;tar -xf - -C \"$d\";cd \"$d\";status=0;bash remote.sh >remote.log 2>&1||status=$?;echo $status >status.txt;tar -cf - --ignore-failed-read status.txt remote.log compile.log native.log frame.json screen.raw native-frame full.* draw0.* draw1.* draw2.* draw3.* draw4.* draw5.* 2>/dev/null"
started=time.monotonic()
with (work/'send.tar').open('rb')as source,(work/'returned.tar').open('wb')as output:
 p=subprocess.run(['ssh','-o','BatchMode=yes','-o','ConnectTimeout=15','coderos-4080',remote],stdin=source,stdout=output,stderr=(work/'ssh.log').open('wb'),timeout=700)
assert p.returncode==0
allowed={'status.txt','remote.log','compile.log','native.log','frame.json','screen.raw','native-frame'}|{name+'.'+ext for name in ['full','draw0','draw1','draw2','draw3','draw4','draw5']for ext in ['raw','raw.target2','raw.target3','raw.target4','json','log']}
with tarfile.open(work/'returned.tar')as archive:
 for member in archive.getmembers():
  assert member.name in allowed and member.isfile();(work/member.name).write_bytes(archive.extractfile(member).read())
status=(work/'status.txt').read_text().strip();receipt={'status':'executed'if status=='0'else'failed','native_status':status,'elapsed_seconds':time.monotonic()-started,'inputs':inputs,'source_fence':{p:hashlib.sha256(Path(p).read_bytes()).hexdigest()==h for p,h in inputs.items()},'compiler_sha256':hashlib.sha256((root/'coax/bin/coax').read_bytes()).hexdigest()};(work/'execution.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps({k:receipt[k]for k in ['status','native_status','elapsed_seconds']}));raise SystemExit(0 if status=='0'else 1)
