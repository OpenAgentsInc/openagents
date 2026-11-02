import hashlib,json,re,subprocess,time,os,signal
from pathlib import Path
from PIL import Image
root=Path('/Users/christopherdavid/work/openagents-aethyr-book')
oldtool=subprocess.check_output(['git','show','HEAD:verse/tools/sans-atlas.coax'],cwd=root,text=True)
oldhost=subprocess.check_output(['git','show','HEAD:coax/web/host.js'],cwd=root,text=True)
block=oldtool.split('parts := [5] string.[',1)[1].split('\n  for p: parts',1)[0]
chunks=[json.loads(s) for s in re.findall(r'"(?:[^"\\]|\\.)*"',block)]
assert len(chunks)==3
extra=re.search(r'^const SANS_EXTRA = \[(.*)\];$',oldhost,re.M).group(1)
font=re.search(r"^const SANS_FONT = '(.*)';$",oldhost,re.M).group(1)
script=chunks[0]+extra+chunks[1]+font+chunks[2]
script_path=Path('/tmp/aethyr-ch11-sans-legacy-script.js');script_path.write_text(script)
shot=Path('/tmp/aethyr-ch11-sans-legacy.png');log=Path('/tmp/aethyr-ch11-sans-legacy-headless.log')
assert not shot.exists()
command=[str(root/'coax/web/tool.sh'),'shot','about:blank',str(shot),'1536x1302','300','--eval',script]
t=time.monotonic()
with log.open('wb') as output:
 p=subprocess.Popen(command,cwd=root,stdout=output,stderr=subprocess.STDOUT,start_new_session=True)
 try:status=p.wait(timeout=120)
 except subprocess.TimeoutExpired:
  os.killpg(p.pid,signal.SIGKILL);p.wait();raise
assert status==0,log.read_text()
im=Image.open(shot).convert('RGBA');assert im.size==(1536,1302)
expected=bytearray();row=0
for width,height in [(1536,672),(768,336),(384,168),(192,84),(96,42)]:
 for y in range(row,row+height):
  for x in range(width):
   r,g,b,a=im.getpixel((x,y));assert r==g==b and a==255;expected.append(r)
 row+=height
actual=Path('/tmp/aethyr-ch11-sans-alpha.bin').read_bytes()
assert bytes(expected)==actual,{'different':sum(a!=b for a,b in zip(expected,actual)),'legacy':len(expected),'new':len(actual)}
sha=lambda b:hashlib.sha256(b).hexdigest()
receipt={'schema':'aethyr-sans-legacy-pixel-parity-v1','status':'pass','bytes':len(actual),'alpha_sha256':sha(actual),'frozen_legacy_tool_sha256':sha(oldtool.encode()),'frozen_legacy_host_sha256':sha(oldhost.encode()),'legacy_script_sha256':sha(script.encode()),'legacy_png_sha256':sha(shot.read_bytes()),'new_script_sha256':sha(Path('/tmp/aethyr-ch11-sans-script.js').read_bytes()),'elapsed_seconds':round(time.monotonic()-t,3),'scope':'Current offline Coax command generation versus frozen legacy JavaScript raster policy, identical alpha bytes for every mip on this machine. This does not qualify the WebGPU-uploaded texture; its actual RGBA capture is separate.'}
Path('/tmp/aethyr-ch11-sans-legacy-parity.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(receipt))
