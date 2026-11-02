from pathlib import Path
import subprocess,tarfile,json,hashlib
root=Path('/Users/christopherdavid/work/openagents-aethyr-book');work=Path('/tmp/aethyr-ch11-native-depth-plane');paths=[work/'depth-plane.c',work/'depth-plane.spv']+[root/'verse/gpu'/n for n in ['vks.c','vks.h','vkc.c','vkc.h']]
with tarfile.open(work/'depth-plane.tar','w')as t:
 for p in paths:t.add(p,arcname=p.name)
remote="set -e;d=$(mktemp -d);trap 'rm -rf \"$d\"' EXIT;tar -xf - -C \"$d\";cd \"$d\";export VK_DRIVER_FILES=/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json;timeout 180 nix-shell -p gcc vulkan-headers vulkan-loader vulkan-validation-layers spirv-tools --run 'set -e; spirv-val --target-env vulkan1.1 depth-plane.spv;gcc -std=c11 -O2 -ffp-contract=off -fno-fast-math -I. depth-plane.c vks.c vkc.c -lvulkan -lm -o probe;export VK_ADD_LAYER_PATH=$(dirname $(find /nix/store -maxdepth 6 -name VkLayer_khronos_validation.json -path \"*validation-layers*\" | head -1));export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation;./probe'"
with (work/'depth-plane.tar').open('rb')as source,(work/'depth-plane-native.log').open('wb')as output:
 p=subprocess.run(['ssh','-o','BatchMode=yes','-o','ConnectTimeout=15','coderos-4080',remote],stdin=source,stdout=output,stderr=subprocess.STDOUT,timeout=240)
assert p.returncode==0,(work/'depth-plane-native.log').read_text()
text=(work/'depth-plane-native.log').read_text();record=json.loads(next(x for x in text.splitlines()if x.startswith('{')));record['source_sha256']={str(p):hashlib.sha256(p.read_bytes()).hexdigest()for p in paths};record['shader_source_sha256']=hashlib.sha256((root/'aethyr/tests/render-depth-plane-probe.coax').read_bytes()).hexdigest();record['compiler_sha256']=hashlib.sha256((root/'coax/bin/coax').read_bytes()).hexdigest();(work/'depth-plane-native.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record))
