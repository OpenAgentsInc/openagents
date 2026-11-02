from pathlib import Path
import hashlib,json,math,struct,subprocess,tarfile,time
root=Path('/Users/christopherdavid/work/openagents-aethyr-book');w=Path(__file__).parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();paths=[w/'gpu.c',w/'probe.spv',w/'cases.bin',w/'probe']+[root/'verse/gpu'/n for n in ['vks.c','vks.h','vkc.c','vkc.h']];sources=[root/'aethyr/render/ring8.coax',root/'apps/openagents-web/gpu/deadgears.coax',root/'aethyr/tests/render-ring8-probe.coax',root/'verse/gpu/shader.coax',root/'verse/gpu/wgsl.coax'];before={str(p):sha(p)for p in paths+sources}
with tarfile.open(w/'gpu-send.tar','w')as t:
 for p in paths:t.add(p,arcname=p.name)
remote="set -e;d=$(mktemp -d);trap 'rm -rf \"$d\"' EXIT;tar -xf - -C \"$d\";cd \"$d\";export VK_DRIVER_FILES=/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json;status=0;timeout 180 nix-shell -p gcc vulkan-headers vulkan-loader vulkan-validation-layers spirv-tools --run 'set -e;spirv-val --target-env vulkan1.1 probe.spv;chmod +x probe;export VK_ADD_LAYER_PATH=$(dirname $(find /nix/store -maxdepth 6 -name VkLayer_khronos_validation.json -path \"*validation-layers*\" | head -1));export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation;./probe' >gpu.log 2>&1||status=$?;echo $status >gpu-status.txt;tar -cf - --ignore-failed-read gpu.log gpu-status.txt observations.rgba32f probe 2>/dev/null"
start=time.monotonic()
with (w/'gpu-send.tar').open('rb')as inp,(w/'gpu-returned.tar').open('wb')as out,(w/'gpu-ssh.log').open('wb')as err:r=subprocess.run(['ssh','-o','BatchMode=yes','-o','ConnectTimeout=15','coderos-4080',remote],stdin=inp,stdout=out,stderr=err,timeout=240)
assert r.returncode==0
with tarfile.open(w/'gpu-returned.tar')as t:
 for m in t.getmembers():assert m.name in {'gpu.log','gpu-status.txt','observations.rgba32f','probe'}and m.isfile();(w/m.name).write_bytes(t.extractfile(m).read())
assert (w/'gpu-status.txt').read_text().strip()=='0',(w/'gpu.log').read_text()[-3000:]
meta=json.loads(next(s for s in(w/'gpu.log').read_text().splitlines()if s.startswith('{')));assert meta['validation_errors']==meta['validation_warnings']==0;assert all(sha(Path(p))==h for p,h in before.items());expected=json.loads((w/'cases.json').read_text())['cases'];data=(w/'observations.rgba32f').read_bytes();word=lambda x:struct.unpack('<I',struct.pack('<f',x))[0]
def compare(raw):
 if len(raw)!=len(expected)*16:return False
 for i,c in enumerate(expected):
  for a,b in zip(struct.unpack_from('<4f',raw,i*16),c['expected']):
   if not math.isfinite(a)or(word(a)!=word(b)and not(a==0 and b==0)):return False
 return True
assert compare(data),'analytic primitive comparison failed'
rows=[struct.unpack_from('<4f',data,i*16)for i in range(len(expected))];seams=[]
for rr in range(6):
 a=rr*48 if rr<5 else 197;b=rr*48+43 if rr<5 else 236
 for selector in range(3):assert struct.pack('<4f',*rows[a*3+selector])==struct.pack('<4f',*rows[b*3+selector]),(rr,selector)
 seams.append({'row':rr,'first':a,'last':b})
# Comparison-loss controls operate only on retained observations, never rerunGPU.
controls={};fault=bytearray(data);i=next(i for i,c in enumerate(expected)if c['selector']==2 and abs(c['expected'][0])>0 and abs(c['expected'][0])<1);value=struct.unpack_from('<I',fault,i*16)[0];struct.pack_into('<I',fault,i*16,value^1);controls['one_ulp_diagonal_loss']=not compare(fault)
controls['truncated_observation']=not compare(data[:-1]);controls['extra_observation']=not compare(data+b'\0'*16);controls['lost_all_outputs']=not compare(b'\0'*len(data));controls['stale_one_case']=not compare(data[:16]*len(expected));fault=bytearray(data);struct.pack_into('<f',fault,3*4,0);controls['lost_alpha']=not compare(fault);fault=bytearray(data);struct.pack_into('<I',fault,0,0x7fc00000);controls['nonfinite']=not compare(fault);assert all(controls.values())
q={'reused_executable_evidence':'probe-before-controlflow/gpu-qualification.json','schema':'aethyr.render.ring8-vulkan-primitive.v1','status':'pass','scope':'Actual GPU primitive/direction packet against input-only analytic expected f32 data; finite nonzero words exact, signed zero numeric-equivalent. Exact actual-output seams are separately bit-checked. Device qualification, not universal arithmetic equivalence.','compiler_sha256':'1fac3750bcffbb735ef26f4a065d16414b911068fd7d68bc3ca112565d2432aa','device':meta,'cases':len(expected),'channels':len(expected)*4,'seams':seams,'comparison_loss_controls':controls,'elapsed_seconds':time.monotonic()-start,'source_fence':before,'executable_sha256':sha(w/'probe'),'observations_sha256':sha(w/'observations.rgba32f'),'cases_sha256':sha(w/'cases.json')};(w/'gpu-qualification.json').write_text(json.dumps(q,indent=2)+'\n');print(json.dumps({k:q[k]for k in ['status','cases','channels','comparison_loss_controls','elapsed_seconds']},indent=2))
