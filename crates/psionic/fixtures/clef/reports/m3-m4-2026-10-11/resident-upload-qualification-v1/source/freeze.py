"""Freeze source and existing inputs without loading a model or reading GGUF payloads."""
import ast,json
from pathlib import Path
import proof
HERE=proof.HERE;ROOT=proof.ROOT;OLD=proof.OLD

def main():
    for name in ('source-manifest.json','inputs.json','qualification-source.json'):proof.require(not (HERE/name).exists(),'refuse replacing '+name)
    old=proof.load(OLD/'source-manifest.json');patch=proof.load(proof.BASE/'resident-positional-upload-patch-v1/frozen-sources.json')
    paths={row['path'] for row in old['files']}|set(patch['sources'])
    rows=[{'path':p,'sha256':proof.sha(ROOT/p)} for p in sorted(paths)]
    oldhash={r['path']:r['sha256'] for r in old['files']};newhash={r['path']:r['sha256'] for r in rows}
    changes=[p for p in sorted(paths) if p.startswith('crates/psionic/crates/') and oldhash.get(p)!=newhash[p]]
    proof.require(set(changes)<=set(patch['sources']),'unexpected product changes: '+str(changes))
    for p,r in patch['sources'].items():proof.require(newhash[p]==r['sha256'],'patch changed: '+p)
    proof.write(HERE/'source-manifest.json',{'baseline_base_commit':old['base_commit'],'build_source_commit':__import__('subprocess').check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'profile':'release','rust_toolchain':'1.97.1','cargo_build_jobs':2,'files':rows,
        'baseline_source_sha256':proof.BASELINE_SOURCE_SHA,'patch_descriptor_sha256':proof.sha(proof.BASE/'resident-positional-upload-patch-v1/frozen-sources.json'),'changed_product_files':changes})
    proof.require(proof.sha(OLD/'psionic-openai-server')==proof.BASELINE_SHA,'baseline binary differs')
    files=[OLD/'source-manifest.json',OLD/'binaries.json',OLD/'guard.json',OLD/'27b-text-check/summary.json',OLD/'27b-text-check/records.jsonl',OLD/'27b-text-check/candidate/rows.jsonl',OLD/'27b-text-check/candidate/rows.f16',OLD/'27b-image-check/run.json',OLD/'27b-image-check/0-images.json',proof.HEAD,
           ROOT/'crates/psionic/fixtures/clef/e2e/requests.jsonl',proof.BASE/'reference27b/images-mps-bf16.json',proof.BASE/'resident-positional-upload-patch-v1/frozen-sources.json',proof.BASE/'resident-positional-upload-patch-v1/changes.patch']
    for name in [f'text-{i:03d}' for i in range(40)]+['long-4k','long-16k']:
        files.extend([OLD/'27b-text-check'/(name+'.request.json'),OLD/'27b-text-check'/('candidate-'+name+'.response.json')])
    cases=proof.helper.image_parity.DEFAULT_CASES;files.append(cases)
    for c in proof.load(cases):files.extend(cases.parent/name for name in c['images'])
    vision=Path('@@CLEF_HOME@@/.ollama/models/manifests/registry.ollama.ai/library/clef/latest');files.append(vision)
    v=proof.load(vision);blobs=vision.parents[4]/'blobs';blobs=Path('@@CLEF_HOME@@/.ollama/models/blobs')
    visionstates={}
    for layer in v['layers']:
        if layer.get('name','').startswith('model.visual.'):
            path=blobs/layer['digest'].replace(':','-');proof.require(path.stat().st_size==layer['size'],'vision blob size differs');visionstates[str(path)]=proof.state(path)
    proof.require(len(visionstates)==333,'vision tensor count changed')
    configpath=blobs/v['config']['digest'].replace(':','-');files.append(configpath)
    image_old=proof.load(OLD/'27b-image-check/run.json')['artifacts']
    proof.require(proof.sha(vision)==image_old['vision_manifest']['sha256'],'vision manifest differs from passed baseline')
    proof.require(proof.sha(proof.HEAD)==image_old['head']['sha256'],'head differs from passed baseline')
    snapshot=proof.state(proof.GGUF);proof.require(snapshot==[19232219200,2660377039,1791700892618502144,1791700892619286481],'GGUF verified snapshot changed')
    proof.write(HERE/'inputs.json',{'files':{str(p):proof.sha(p) for p in sorted(set(files))},'baseline_binary_sha256':proof.BASELINE_SHA,
        'gguf_state':snapshot,'gguf_sha256':image_old['gguf']['sha256'],'gguf_verification':'historically SHA-256 verified; unchanged inode/size/mtime/ctime, no 19 GB rehash',
        'vision_blob_states':visionstates,'vision_identity_limit':'Manifest digests match the passed image baseline; current 333 tensor file snapshots are frozen. This preparation does not independently rehash all vision payloads.'})
    scripts=list(HERE.glob('*.py'));dependencies=[ROOT/'crates/psionic/fixtures/clef/tools'/name for name in ['mac_quiet_bench.py','mac_memory.py','image_parity.py','e2e.py']]
    for p in scripts+dependencies:ast.parse(p.read_text())
    files=scripts+dependencies+[HERE/'source-manifest.json',HERE/'inputs.json',HERE/'images-config.json',HERE/'PLAN.md',HERE/'negative-checks.json']
    proof.write(HERE/'qualification-source.json',{'status':'frozen_unrun','files':{str(p):proof.sha(p) for p in sorted(files)}})
    print(json.dumps({'status':'frozen_unrun','source_files':len(rows),'qualification_sha256':proof.sha(HERE/'qualification-source.json'),'changed_product_files':changes},indent=2))
if __name__=='__main__':main()
