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
if a.reuse_executable:
 previous=a.reuse_executable.parent/'stage'
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
./native-frame frame.config package/render.json screen.raw frame.json rgba16f >native.log 2>&1
'
''')
if a.reuse_executable:
 remote_text=(stage/'remote.sh').read_text();remote_text='\n'.join('chmod +x native-frame'if line.startswith('gcc ')else line for line in remote_text.splitlines())+'\n';(stage/'remote.sh').write_text(remote_text)
if a.skip_spv_validation:
 text=(stage/'remote.sh').read_text();text='\n'.join(line for line in text.splitlines()if not line.startswith('for p in package/'))+'\n';(stage/'remote.sh').write_text(text)
with tarfile.open(work/'send.tar','w')as archive:
 for p in stage.rglob('*'):
  if p.is_file():archive.add(p,arcname=str(p.relative_to(stage)))
remote="set -e;d=$(mktemp -d);trap 'rm -rf \"$d\"' EXIT;tar -xf - -C \"$d\";cd \"$d\";status=0;bash remote.sh >remote.log 2>&1||status=$?;echo $status >status.txt;tar -cf - --ignore-failed-read status.txt remote.log compile.log native.log frame.json screen.raw native-frame 2>/dev/null"
started=time.monotonic()
with (work/'send.tar').open('rb')as source,(work/'returned.tar').open('wb')as output:
 p=subprocess.run(['ssh','-o','BatchMode=yes','-o','ConnectTimeout=15','coderos-4080',remote],stdin=source,stdout=output,stderr=(work/'ssh.log').open('wb'),timeout=700)
assert p.returncode==0
allowed={'status.txt','remote.log','compile.log','native.log','frame.json','screen.raw','native-frame'}
with tarfile.open(work/'returned.tar')as archive:
 for member in archive.getmembers():
  assert member.name in allowed and member.isfile();(work/member.name).write_bytes(archive.extractfile(member).read())
status=(work/'status.txt').read_text().strip();receipt={'status':'executed'if status=='0'else'failed','native_status':status,'elapsed_seconds':time.monotonic()-started,'inputs':inputs,'source_fence':{p:hashlib.sha256(Path(p).read_bytes()).hexdigest()==h for p,h in inputs.items()},'compiler_sha256':hashlib.sha256((root/'coax/bin/coax').read_bytes()).hexdigest()};(work/'execution.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps({k:receipt[k]for k in ['status','native_status','elapsed_seconds']}));raise SystemExit(0 if status=='0'else 1)
