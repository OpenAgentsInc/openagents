from pathlib import Path
import hashlib,json,subprocess
root=Path(__file__).parent
source=root/'combined.metal'
source.write_text((root/'baseline.metal').read_text()+'\n'+(root/'candidates.metal').read_text())
commands=[['xcrun','-sdk','macosx','metal','-std=metal4.0','-c',str(source),'-o',str(root/'kernels.air')],['xcrun','swiftc','-O','-num-threads','2',str(root/'probe.swift'),'-o',str(root/'probe')]]
for command in commands:
 print(json.dumps({'command':command}),flush=True)
 subprocess.run(command,check=True)
print(json.dumps({'compiled':True,'gpu_executed':False,'binary_sha256':hashlib.sha256((root/'probe').read_bytes()).hexdigest()}),flush=True)
