from pathlib import Path
import subprocess,tarfile,hashlib,json,shutil,time,argparse
p=argparse.ArgumentParser();p.add_argument('out',type=Path);p.add_argument('--only',choices=['haunt','sandbox']);p.add_argument('--null',action='store_true');a=p.parse_args();root=Path('/Users/christopherdavid/work/openagents-aethyr-book');out=a.out;out.mkdir(exist_ok=True);stage=out/'stage';stage.mkdir(exist_ok=True);base=Path('/tmp/aethyr-ch11-native-static-draw-producer');hashes={}
paths=[p for p in base.iterdir()if p.is_file()and(p.suffix in ['.c','.h']or p.name=='sources')]+[root/'verse/native'/n for n in ['render-realm.c','render-realm.h','render-host.h','render-textures.h','render-media.h','face.h','realm-host.c','realm-host.h']]+[root/'verse/gpu'/n for n in ['vks.h','vkc.h']]+[root/'aethyr/render/tests/frame-input.h']+[Path('/tmp/aethyr-ch11-native-mesh-placeholder')/n for n in ['RenderHost.c','RenderHost.h']]+[Path('/tmp/aethyr-ch11-native-static-draw/check.c'),Path('/tmp/aethyr-ch11-native-build-garden-allocation/render-native/RenderTexture.h')]
for f in paths:(stage/f.name).write_bytes(f.read_bytes());hashes[str(f)]=hashlib.sha256(f.read_bytes()).hexdigest()
# Face code is a platform source, not generated C: stage the current owner.
f=root/'apps/openagents-web/realm-face.c';(stage/'realm-face.c').write_bytes(f.read_bytes());hashes[str(f)]=hashlib.sha256(f.read_bytes()).hexdigest()
for slug in ['haunted-house','sandbox']:
 f=Path('/tmp/aethyr-ch11-producer-captures')/(slug+'.pack');(stage/f.name).write_bytes(f.read_bytes());hashes[str(f)]=hashlib.sha256(f.read_bytes()).hexdigest()
(stage/'remote.sh').write_text('''set -euo pipefail
timeout 500 nix-shell -p gcc vulkan-headers --run '
set -e
read -r -a sources <sources
gcc -std=gnu11 -O2 -no-pie -ffunction-sections -fdata-sections -Wl,--gc-sections -ffp-contract=off -fno-fast-math -w -I. check.c "${sources[@]}" realm-host.c RenderHost.c -lm -o check >compile.log 2>&1
./check haunted-house.pack >haunt.log 2>&1
./check sandbox.pack >sandbox.log 2>&1
'
''')
if a.null:
 f=Path('/tmp/aethyr-ch11-native-static-draw/null.c');(stage/'check.c').write_bytes(f.read_bytes());hashes[str(f)]=hashlib.sha256(f.read_bytes()).hexdigest();text=(stage/'remote.sh').read_text().replace('read -r -a sources <sources','').replace('\"${sources[@]}\"','').replace('\"${sources[@]}\"','').replace('\"${sources[@]}\"','').replace('\"${sources[@]}\"','')
 text=text.replace(chr(34)+'${sources[@]}'+chr(34),'').replace('./check haunted-house.pack >haunt.log 2>&1','./check >null.log 2>&1').replace('./check sandbox.pack >sandbox.log 2>&1','');(stage/'remote.sh').write_text(text)
if a.only:
 text=(stage/'remote.sh').read_text();omit='./check sandbox.pack >sandbox.log 2>&1'if a.only=='haunt'else'./check haunted-house.pack >haunt.log 2>&1';(stage/'remote.sh').write_text(text.replace(omit,''))
with tarfile.open(out/'send.tar','w')as t:
 for f in stage.iterdir():
  if f.is_file():t.add(f,arcname=f.name)
remote="set -e;d=$(mktemp -d);trap 'rm -rf \"$d\"' EXIT;tar -xf - -C \"$d\";cd \"$d\";status=0;bash remote.sh >remote.log 2>&1||status=$?;echo $status>status.txt;tar -cf - --ignore-failed-read status.txt remote.log compile.log haunt.log sandbox.log null.log check 2>/dev/null"
start=time.monotonic()
with(out/'send.tar').open('rb')as i,(out/'returned.tar').open('wb')as o,(out/'ssh.log').open('wb')as e:r=subprocess.run(['ssh','-o','BatchMode=yes','coderos-4080',remote],stdin=i,stdout=o,stderr=e,timeout=600)
assert r.returncode==0
with tarfile.open(out/'returned.tar')as t:
 for m in t.getmembers():assert m.isfile()and'/'not in m.name;(out/m.name).write_bytes(t.extractfile(m).read())
status=int((out/'status.txt').read_text());record={'native_exit':status,'elapsed_seconds':time.monotonic()-start,'inputs_sha256':hashes,'source_fence_unchanged':all(hashlib.sha256(Path(f).read_bytes()).hexdigest()==h for f,h in hashes.items()),'executable_sha256':hashlib.sha256((out/'check').read_bytes()).hexdigest()if(out/'check').exists()else None};(out/'qualification.json').write_text(json.dumps(record,indent=2)+'\n');print(record['native_exit'],record['elapsed_seconds']);raise SystemExit(status)
