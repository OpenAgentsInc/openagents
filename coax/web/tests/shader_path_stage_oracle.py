# Firm reason: 2 (independent oracle of the deploy wrapper's staged shader bytes and page URLs).
import hashlib, json, re, shutil, subprocess, sys, tempfile
from pathlib import Path

repo=Path(__file__).resolve().parents[3]
script=(repo/'coax/web/deploy.sh').read_text().split("<<'PY'\n",1)[1].split('\nPY\n',1)[0]
with tempfile.TemporaryDirectory(prefix='character-shader-stage-') as temp:
    root=Path(temp); app=root/'app'; stage=root/'stage'; (app/'build').mkdir(parents=True)
    shutil.copyfile(repo/'apps/openagents-web/build/fivelab.wasm',app/'build/fivelab.wasm')
    for name in ['fivelab','charactercreator']:
        shutil.copyfile(repo/f'apps/openagents-web/{name}.wgsl',app/f'{name}.wgsl')
    (app/'llms.txt').write_text('Shader staging fixture\n')
    (root/'host.js').write_text('// stage fixture\n')
    pages={'index.html':None,'realms/character-creator/index.html':'/charactercreator.wgsl'}
    for path,declared in pages.items():
        p=app/path; p.parent.mkdir(parents=True,exist_ok=True)
        extra=f' data-wgsl="{declared}"' if declared else ''
        p.write_text(f'<script type="module" src="/coaxial/host.js" data-app="fivelab"{extra}></script>')
    subprocess.run([sys.executable,'-c',script,str(app),str(root/'host.js'),str(stage),str(repo/'coax/web')],cwd=repo,check=True,capture_output=True,text=True)
    receipt={}
    for path,declared in pages.items():
        html=(stage/'site'/path).read_text(); urls=re.findall(r'data-wgsl="([^"]+)"',html)
        assert len(urls)==1, 'Page must have one shader attribute after staging'
        expected=repo/'apps/openagents-web'/('charactercreator.wgsl' if declared else 'fivelab.wgsl')
        copied=stage/'site'/urls[0].lstrip('/')
        assert copied.read_bytes()==expected.read_bytes()
        digest=hashlib.sha256(expected.read_bytes()).hexdigest()
        assert digest[:12] in copied.name
        receipt[path]={'url':urls[0],'sha256':digest,'bytes':copied.stat().st_size,'shader_attributes':len(urls)}
    print(json.dumps(receipt,indent=2))
