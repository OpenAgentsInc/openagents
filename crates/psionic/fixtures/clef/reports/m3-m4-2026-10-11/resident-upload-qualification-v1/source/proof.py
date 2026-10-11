"""Frozen inputs and exact execution scope for positional resident upload."""
import json
import os
from pathlib import Path
import acceptance_helpers as helper

HERE=Path(__file__).resolve().parent
BASE=HERE.parent
OLD=BASE/'attention-qk128-key32-integration'
ROOT=helper.REPO
bench=helper.bench
BASELINE_SHA='f4def388b4b6622fef9023c2616639c215829d7a519359eb64d1dbca9fdfa31f'
BASELINE_SOURCE_SHA='843d7861b221b650d17f4a4902c2297117765e583a2d6159dce6ae797da70a09'
GGUF=BASE/'artifacts/Clef-Q4_K_M.gguf'
HEAD=BASE/'artifacts/clef-bf16/joint_head.safetensors'
ENV={'PSIONIC_CLEF_METAL_FFN_WEIGHTS':'resident-f16','PSIONIC_CLEF_METAL_WEIGHT_UPLOAD':'positional',
     'PSIONIC_CLEF_METAL_ATTENTION':'fused-tensorops','PSIONIC_CLEF_METAL_GEMM':'weight-shapes',
     'PSIONIC_CLEF_METAL_HEAD':'fused-fields','PSIONIC_CLEF_METAL_DELTA':'sequential',
     'PSIONIC_CLEF_PREFIX_CACHE_BYTES':'0','PSIONIC_CLEF_BATCH_REQUESTS':'1',
     'RAYON_NUM_THREADS':'2','OMP_NUM_THREADS':'2','VECLIB_MAXIMUM_THREADS':'2'}

def require(value,message):
    if not value:raise ValueError(message)
def load(path):return json.loads(Path(path).read_text())
def sha(path):return bench.file_sha(path)
def write(path,value):helper.write(path,value)
def state(path):
    s=path.stat();return [s.st_size,s.st_ino,s.st_mtime_ns,s.st_ctime_ns]
def verify_files(files):
    for path,digest in files.items():require(sha(path)==digest,'frozen file changed: '+path)

def frozen():
    data=load(HERE/'inputs.json');verify_files(data['files'])
    require(data['baseline_binary_sha256']==BASELINE_SHA and sha(OLD/'source-manifest.json')==BASELINE_SOURCE_SHA,'baseline identity differs')
    require(state(GGUF)==data['gguf_state'],'GGUF differs from prior verified snapshot')
    require(data['gguf_sha256']=='9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822','GGUF digest differs')
    for path,snapshot in data.get('vision_blob_states',{}).items():require(state(Path(path))==snapshot,'vision blob snapshot changed: '+path)
    sources=load(HERE/'qualification-source.json');verify_files(sources['files'])
    return data,sources

def candidate():
    data=load(HERE/'candidate.json')
    require(data['status']=='complete' and data['source_manifest_unchanged'] is True,'candidate build is incomplete')
    require(Path(data['binary']).resolve().is_relative_to(HERE),'candidate must be copied into this scratch directory')
    require(sha(data['binary'])==data['binary_sha256'],'candidate binary changed')
    fixture=data['fixture_binary'];require(Path(fixture['path']).resolve().is_relative_to(HERE) and sha(fixture['path'])==fixture['sha256'],'fixture binary changed')
    require(sha(data['build_guard'])==data['build_guard_sha256'],'build guard changed')
    guard=load(data['build_guard'])
    require(guard['harness_manifest_sha256']==sha(HERE/'qualification-source.json'),'built harness identity differs')
    require(guard['status']=='complete' and guard['sources_rechecked'] and guard['memory']['owned_group_stopped']
            and not any(guard['memory'].get(k) for k in ('memory_rejection','sampling_error','cleanup_error')),'build guard did not pass')
    require(sha(data['source_manifest'])==data['source_manifest_sha256']==guard['source_manifest_sha256'],'candidate source manifest differs')
    current=load(data['source_manifest']);old=load(OLD/'source-manifest.json')
    a={v['path']:v['sha256'] for v in old['files']};b={v['path']:v['sha256'] for v in current['files']}
    for path,digest in b.items():require(sha(ROOT/path)==digest,'built source changed: '+path)
    product=lambda p:p.startswith('crates/psionic/crates/')
    changed={p for p in a.keys()|b.keys() if product(p) and a.get(p)!=b.get(p)}
    allowed={'crates/psionic/crates/psionic-catalog/src/lib.rs','crates/psionic/crates/psionic-models/src/lib.rs',
             'crates/psionic/crates/psionic-serve/src/qwen35/clef_metal.rs',
             'crates/psionic/crates/psionic-serve/src/qwen35/clef_metal/tests.rs',
             'crates/psionic/crates/psionic-serve/src/qwen35/clef_metal/tests/weight_upload.rs'}
    require(changed<=allowed,'unexpected product changes: '+str(sorted(changed-allowed)))
    require(all(p in b for p in a if product(p)),'product source disappeared')
    critical=[p for p in a if '/psionic-serve/src/clef/' in p or '/psionic-backend-metal/' in p]
    require(critical and all(a[p]==b.get(p) for p in critical),'encoder/head/backend arithmetic sources changed')
    data['source_audit']={'changed_product_files':sorted(changed),'unchanged_encoder_head_backend_files':len(critical),
        'baseline_weight_placement':'q4k-on-demand','candidate_weight_placement':'resident-f16 plus positional upload',
        'identity_scope':'Placement differs deliberately. Exact output bits are required; equivalence is not assumed.'}
    return data

def limits():
    return {'measurement_mode':'shared','max_start_load':150,'max_run_load':180,'rounds':1,'runs':1,
            'max_rss_gib':52,'minimum_available_memory_gib':12,'startup_timeout':600,'request_timeout':300,
            'deadline_seconds':1800,'model_path':str(GGUF),'clef_head':str(HEAD)}

def clean_inherited_environment():
    for key in list(os.environ):
        if key.startswith(('DYLD_','PYTORCH_MPS_')) or key in ('LD_PRELOAD','LD_LIBRARY_PATH'):
            os.environ.pop(key)

def require_leases():
    leases=set(os.environ.get('OPENAGENTS_LEASES','').split(','))
    require({'gpu','memory','disk'}<=leases and not {'quiet','build'}&leases,'GPU/memory/disk without quiet or build required')


def disk_check(start=False,build=False):
    import shutil
    required=(31 if build else 27) if start else 25
    require(shutil.disk_usage(HERE).free>=required*1024**3,'free-disk floor refused')
    size=sum(p.stat().st_size for p in HERE.rglob('*') if p.is_file())
    require(size<=2*1024**3,'qualification output exceeds 2 GiB')
    return {'free_bytes':shutil.disk_usage(HERE).free,'output_bytes':size}
