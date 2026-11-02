from pathlib import Path
import hashlib,json,shutil,struct,subprocess,tarfile,time
root=Path('/Users/christopherdavid/work/openagents-aethyr-book');old=Path('/tmp/aethyr-ch11-native-clip-output');w=Path('/tmp/aethyr-ch11-native-icecream-clip');w.mkdir(exist_ok=True);src=Path('/tmp/aethyr-ch11-icecream-clip-output-cases.json');cases=json.loads(src.read_text());sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();base=json.loads((old/'qualification.json').read_text());assert sha(old/'probe')==base['executable_sha256'];assert cases['width']==320 and cases['height']==180 and len(cases['cases'])==8
for n in ['probe','probe.spv']:shutil.copy2(old/n,w/n)
shutil.copy2(src,w/'cases.json');(w/'cases.bin').write_bytes(b''.join(struct.pack('<II12f',c['x'],c['y'],*[n for v in c['clips']for n in v])for c in cases['cases']))
paths=[w/n for n in ['probe','probe.spv','cases.bin']];before={str(p):sha(p)for p in paths}
with tarfile.open(w/'send.tar','w')as t:
 for p in paths:t.add(p,arcname=p.name)
remote="set -e;d=$(mktemp -d);trap 'rm -rf \"$d\"' EXIT;tar -xf - -C \"$d\";cd \"$d\";export VK_DRIVER_FILES=/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json;status=0;timeout 180 nix-shell -p vulkan-loader vulkan-validation-layers --run 'set -e;chmod +x probe;export VK_ADD_LAYER_PATH=$(dirname $(find /nix/store -maxdepth 6 -name VkLayer_khronos_validation.json -path \"*validation-layers*\" | head -1));export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation;./probe' >native.log 2>&1||status=$?;echo $status>status.txt;tar -cf - --ignore-failed-read native.log status.txt observations.rgba32f 2>/dev/null"
start=time.monotonic()
with (w/'send.tar').open('rb')as inp,(w/'returned.tar').open('wb')as out,(w/'ssh.log').open('wb')as err:r=subprocess.run(['ssh','-o','BatchMode=yes','-o','ConnectTimeout=15','coderos-4080',remote],stdin=inp,stdout=out,stderr=err,timeout=240)
assert r.returncode==0
with tarfile.open(w/'returned.tar')as t:
 for m in t.getmembers():assert m.name in {'native.log','status.txt','observations.rgba32f'}and m.isfile();(w/m.name).write_bytes(t.extractfile(m).read())
assert (w/'status.txt').read_text().strip()=='0',(w/'native.log').read_text()[-3000:]
q=json.loads(next(s for s in(w/'native.log').read_text().splitlines()if s.startswith('{')));assert q['validation_errors']==q['validation_warnings']==0;assert all(sha(Path(p))==h for p,h in before.items());q.update(schema='aethyr.render.native-icecream-exact-clip-probe.v1',scope=cases['scope'],cases_sha256=sha(src),input_fence=before,elapsed_seconds=time.monotonic()-start,executable_sha256=sha(w/'probe'),reused_executable_evidence='docs/verification/data/aethyr-ch11-2026-11-01/native-media/exact-clip-output-probe/native-inputs-and-executable.tar.gz');q['observations']=[dict(c,rgba=rgba)for c,rgba in zip(cases['cases'],q['observations'])];(w/'qualification.json').write_text(json.dumps(q,indent=2)+'\n');print(json.dumps({'status':'pass','depths':[c['rgba'][0]for c in q['observations']],'elapsed_seconds':q['elapsed_seconds']}))
