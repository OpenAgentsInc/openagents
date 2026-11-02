from pathlib import Path
import hashlib,json,shutil,struct,subprocess,tarfile,time
root=Path('/Users/christopherdavid/work/openagents-aethyr-book');w=Path('/tmp/aethyr-ch11-native-clip-output');w.mkdir(exist_ok=True)
cases_path=Path('/tmp/aethyr-ch11-clip-output-cases.json');cases=json.loads(cases_path.read_text());assert cases['width']==320 and cases['height']==180 and len(cases['cases'])==8
shutil.copy2(cases_path,w/'cases.json');shutil.copy2('/tmp/aethyr-clip-output-probe.spv',w/'probe.spv');data=b''.join(struct.pack('<II12f',c['x'],c['y'],*[n for v in c['clips']for n in v])for c in cases['cases']);(w/'cases.bin').write_bytes(data)
(w/'probe.c').write_text('''#include "vks.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
typedef struct {VksPipe *pipe;VksPass *pass;} Probe;
static void record(VkCommandBuffer cmd,void *user){Probe *p=user;vks_pipe_record(p->pipe,cmd,p->pass);}
int main(void){FILE *f=fopen("probe.spv","rb");assert(f);fseek(f,0,SEEK_END);size_t n=(size_t)ftell(f);rewind(f);uint32_t *words=malloc(n);assert(fread(words,1,n,f)==n&&!fclose(f));
 Vkc v;vkc_open(&v);VkcBuffer u=vkc_buffer(&v,48);VksImage target=vks_image(&v,320,180,1,0,VK_FORMAT_R32G32B32A32_SFLOAT,VKS_TARGET);
 VksBinding binding={.kind=VKS_STORAGE,.buffer=&u,.range=48,.stages=VK_SHADER_STAGE_VERTEX_BIT};VksPass pass={.words=words,.count=n/4,.bindings=&binding,.nb=1,.vertex_count=3,.instance_count=1,.targets={&target},.ntargets=1,.clear={-1,-1,-1,-1}};
 float *pixels=malloc(320*180*16);f=fopen("cases.bin","rb");assert(f);FILE *raw=fopen("observations.rgba32f","wb");assert(raw);printf("{\\"device\\":\\"%s\\",\\"subPixelPrecisionBits\\":%u,\\"observations\\":[",v.props.deviceName,v.props.limits.subPixelPrecisionBits);
 for(int k=0;k<8;k++){uint32_t xy[2];assert(fread(xy,4,2,f)==2&&fread(u.map,4,12,f)==12);VksPipe pipe;vks_pipe_open(&v,&pipe,&pass);Probe probe={&pipe,&pass};vks_submit(&v,record,&probe);vks_read(&v,&target,pixels,320*180*16);float *q=pixels+(xy[1]*320+xy[0])*4;assert(fwrite(q,4,4,raw)==4);printf("%s[%.17g,%.17g,%.17g,%.17g]",k?",":"",(double)q[0],(double)q[1],(double)q[2],(double)q[3]);vks_pipe_close(&v,&pipe);}
 assert(fgetc(f)==EOF&&!fclose(f)&&!fclose(raw));printf("],\\"validation_errors\\":%d,\\"validation_warnings\\":%d}\\n",vkc_validation_errors,vkc_validation_warnings);free(pixels);vks_image_free(&v,&target);vkc_free(&v,&u);vkc_close(&v);free(words);return vkc_validation_errors||vkc_validation_warnings?1:0;}
''')
paths=[w/'probe.c',w/'probe.spv',w/'cases.bin']+[root/'verse/gpu'/n for n in ['vks.c','vks.h','vkc.c','vkc.h']];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();before={str(p):sha(p)for p in paths};before[str(root/'aethyr/tests/render-clip-output-probe.coax')]=sha(root/'aethyr/tests/render-clip-output-probe.coax')
with tarfile.open(w/'send.tar','w')as t:
 for p in paths:t.add(p,arcname=p.name)
remote="set -e;d=$(mktemp -d);trap 'rm -rf \"$d\"' EXIT;tar -xf - -C \"$d\";cd \"$d\";export VK_DRIVER_FILES=/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json;status=0;timeout 180 nix-shell -p gcc vulkan-headers vulkan-loader vulkan-validation-layers spirv-tools --run 'set -e;spirv-val --target-env vulkan1.1 probe.spv;gcc -std=c11 -O2 -ffp-contract=off -fno-fast-math -I. probe.c vks.c vkc.c -lvulkan -lm -o probe;export VK_ADD_LAYER_PATH=$(dirname $(find /nix/store -maxdepth 6 -name VkLayer_khronos_validation.json -path \"*validation-layers*\" | head -1));export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation;./probe' >native.log 2>&1||status=$?;echo $status>status.txt;tar -cf - --ignore-failed-read native.log status.txt observations.rgba32f probe 2>/dev/null"
start=time.monotonic()
with (w/'send.tar').open('rb')as inp,(w/'returned.tar').open('wb')as out,(w/'ssh.log').open('wb')as err:r=subprocess.run(['ssh','-o','BatchMode=yes','-o','ConnectTimeout=15','coderos-4080',remote],stdin=inp,stdout=out,stderr=err,timeout=240)
assert r.returncode==0
with tarfile.open(w/'returned.tar')as t:
 for m in t.getmembers():assert m.name in {'native.log','status.txt','observations.rgba32f','probe'}and m.isfile();(w/m.name).write_bytes(t.extractfile(m).read())
assert (w/'status.txt').read_text().strip()=='0',(w/'native.log').read_text()[-3000:]
q=json.loads(next(s for s in(w/'native.log').read_text().splitlines()if s.startswith('{')));assert q['validation_errors']==q['validation_warnings']==0;assert all(sha(Path(p))==h for p,h in before.items());q.update(schema='aethyr.render.native-exact-clip-probe.v1',scope=cases['scope'],cases_sha256=sha(cases_path),source_fence=before,elapsed_seconds=time.monotonic()-start,executable_sha256=sha(w/'probe'));q['observations']=[{'case':i,'x':c['x'],'y':c['y'],'instance':c['instance'],'ids':c['ids'],'clips':c['clips'],'rgba':rgba}for i,(c,rgba)in enumerate(zip(cases['cases'],q['observations']))];(w/'qualification.json').write_text(json.dumps(q,indent=2)+'\n');print(json.dumps({'status':'pass','observations':q['observations'],'elapsed_seconds':q['elapsed_seconds']},indent=2))
