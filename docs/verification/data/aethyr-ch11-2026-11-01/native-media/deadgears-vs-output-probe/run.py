from pathlib import Path
import hashlib,json,shutil,struct,subprocess,tarfile,time
root=Path('/Users/christopherdavid/work/openagents-aethyr-book');w=Path('/tmp/aethyr-ch11-native-deadgears-vs');w.mkdir(exist_ok=True);src=Path('/tmp/aethyr-ch11-deadgears-vs-cases.json');cases=json.loads(src.read_text());sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();assert len(cases['cases'])==96
shutil.copy2(src,w/'cases.json');shutil.copy2('/tmp/aethyr-deadgears-vs-probe.spv',w/'probe.spv')
for row in cases['source_inputs']:assert sha(root/row['path'])==row['sha256']
for name,row in cases['resources'].items():p=Path(row['file']);assert p.stat().st_size==row['bytes']and sha(p)==row['sha256'];shutil.copy2(p,w/(name+'.bin'))
(w/'cases.bin').write_bytes(b''.join(struct.pack('<4f',*c['uniform'])for c in cases['cases']))
(w/'probe.c').write_text('''#include "vks.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
typedef struct {VksPipe *pipe;VksPass *pass;} Probe;
static void record(VkCommandBuffer cmd,void *user){Probe *p=user;vks_pipe_record(p->pipe,cmd,p->pass);}
static void *load(const char *name,size_t *bytes){FILE *f=fopen(name,"rb");assert(f);fseek(f,0,SEEK_END);*bytes=(size_t)ftell(f);rewind(f);void *p=malloc(*bytes);assert(p&&fread(p,1,*bytes,f)==*bytes&&!fclose(f));return p;}
int main(void){size_t spv_bytes,u_bytes,object_bytes;uint32_t *words=load("probe.spv",&spv_bytes);void *ub=load("u.bin",&u_bytes),*ob=load("objects.bin",&object_bytes);
 Vkc v;vkc_open(&v);VkcBuffer u=vkc_buffer(&v,u_bytes),objects=vkc_buffer(&v,object_bytes),params=vkc_buffer(&v,16);memcpy(u.map,ub,u_bytes);memcpy(objects.map,ob,object_bytes);VksImage target=vks_image(&v,64,64,1,0,VK_FORMAT_R32G32B32A32_SFLOAT,VKS_TARGET);
 VksBinding bindings[3]={{.kind=VKS_STORAGE,.buffer=&u,.range=u_bytes,.stages=VK_SHADER_STAGE_VERTEX_BIT},{.kind=VKS_STORAGE,.buffer=&objects,.range=object_bytes,.stages=VK_SHADER_STAGE_VERTEX_BIT},{.kind=VKS_STORAGE,.buffer=&params,.range=16,.stages=VK_SHADER_STAGE_VERTEX_BIT|VK_SHADER_STAGE_FRAGMENT_BIT}};
 VksPass pass={.words=words,.count=spv_bytes/4,.bindings=bindings,.nb=3,.vertex_count=3,.instance_count=1,.targets={&target},.ntargets=1,.clear={-1,-1,-1,-1}};VksPipe pipe;vks_pipe_open(&v,&pipe,&pass);Probe probe={&pipe,&pass};
 float *pixels=malloc(64*64*16);FILE *f=fopen("cases.bin","rb"),*raw=fopen("observations.rgba32f","wb");assert(f&&raw);printf("{\\"device\\":\\"%s\\",\\"subPixelPrecisionBits\\":%u,\\"observations\\":[",v.props.deviceName,v.props.limits.subPixelPrecisionBits);
 for(int k=0;k<96;k++){assert(fread(params.map,4,4,f)==4);vks_submit(&v,record,&probe);vks_read(&v,&target,pixels,64*64*16);float *q=pixels+(9*64+9)*4;assert(fwrite(q,4,4,raw)==4);printf("%s[%.17g,%.17g,%.17g,%.17g]",k?",":"",(double)q[0],(double)q[1],(double)q[2],(double)q[3]);}
 assert(fgetc(f)==EOF&&!fclose(f)&&!fclose(raw));printf("],\\"validation_errors\\":%d,\\"validation_warnings\\":%d}\\n",vkc_validation_errors,vkc_validation_warnings);free(pixels);vks_pipe_close(&v,&pipe);vks_image_free(&v,&target);vkc_free(&v,&u);vkc_free(&v,&objects);vkc_free(&v,&params);vkc_close(&v);free(words);free(ub);free(ob);return vkc_validation_errors||vkc_validation_warnings?1:0;}
''')
paths=[w/n for n in ['probe.c','probe.spv','cases.bin','u.bin','objects.bin']]+[root/'verse/gpu'/n for n in ['vks.c','vks.h','vkc.c','vkc.h']];before={str(p):sha(p)for p in paths};before[str(root/'aethyr/tests/render-deadgears-vs-probe.coax')]=sha(root/'aethyr/tests/render-deadgears-vs-probe.coax')
with tarfile.open(w/'send.tar','w')as t:
 for p in paths:t.add(p,arcname=p.name)
remote="set -e;d=$(mktemp -d);trap 'rm -rf \"$d\"' EXIT;tar -xf - -C \"$d\";cd \"$d\";export VK_DRIVER_FILES=/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json;status=0;timeout 180 nix-shell -p gcc vulkan-headers vulkan-loader vulkan-validation-layers spirv-tools --run 'set -e;spirv-val --target-env vulkan1.1 probe.spv;gcc -std=c11 -O2 -ffp-contract=off -fno-fast-math -I. probe.c vks.c vkc.c -lvulkan -lm -o probe;export VK_ADD_LAYER_PATH=$(dirname $(find /nix/store -maxdepth 6 -name VkLayer_khronos_validation.json -path \"*validation-layers*\" | head -1));export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation;./probe' >native.log 2>&1||status=$?;echo $status>status.txt;tar -cf - --ignore-failed-read native.log status.txt observations.rgba32f probe 2>/dev/null"
start=time.monotonic()
with (w/'send.tar').open('rb')as inp,(w/'returned.tar').open('wb')as out,(w/'ssh.log').open('wb')as err:r=subprocess.run(['ssh','-o','BatchMode=yes','-o','ConnectTimeout=15','coderos-4080',remote],stdin=inp,stdout=out,stderr=err,timeout=240)
assert r.returncode==0
with tarfile.open(w/'returned.tar')as t:
 for m in t.getmembers():assert m.name in {'native.log','status.txt','observations.rgba32f','probe'}and m.isfile();(w/m.name).write_bytes(t.extractfile(m).read())
assert (w/'status.txt').read_text().strip()=='0',(w/'native.log').read_text()[-3000:]
q=json.loads(next(s for s in(w/'native.log').read_text().splitlines()if s.startswith('{')));assert q['validation_errors']==q['validation_warnings']==0;assert all(sha(Path(p))==h for p,h in before.items());q.update(schema='aethyr.render.native-production-vs-probe.v1',scope=cases['scope'],cases_sha256=sha(src),source_fence=before,elapsed_seconds=time.monotonic()-start,executable_sha256=sha(w/'probe'));q['observations']=[dict(c,rgba=rgba)for c,rgba in zip(cases['cases'],q['observations'])];(w/'qualification.json').write_text(json.dumps(q,indent=2)+'\n');clips=[{'case':i,'instance':c['instance'],'vertex':c['vertex'],'cpu_clip':c['cpu_clip'],'gpu_clip':c['rgba']}for i,c in enumerate(q['observations'])if c['selector']==0];print(json.dumps({'status':'pass','cases':len(q['observations']),'clip_results':clips,'elapsed_seconds':q['elapsed_seconds']},indent=2))
