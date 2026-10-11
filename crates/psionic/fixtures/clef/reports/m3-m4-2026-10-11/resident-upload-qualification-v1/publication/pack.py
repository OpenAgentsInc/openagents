"""Publish exact qualification receipts, splitting large UTF-8 files losslessly."""
import importlib.util,json
from pathlib import Path
HERE=Path(__file__).resolve().parent.parent;BASE=HERE.parent
spec=importlib.util.spec_from_file_location('publication',BASE/'same-q4-runtime-v1/publication/pack.py')
pub=importlib.util.module_from_spec(spec);spec.loader.exec_module(pub)
# Broader sibling paths and the lease owner ID also need reversible substitutions.
pub.REPLACEMENTS.insert(-1,(str(BASE.parent),'@@CLEF_'+'SESSION_ROOT@@'))
lease_session=json.loads((HERE/'build-lease.json').read_text())['holder']['session']
pub.REPLACEMENTS.append((lease_session,'@@CLEF_'+'LEASE_SESSION@@'))
ROOT=pub.ROOT;OUT=HERE/'publishable-v2';LIMIT=900_000

def load(p):return json.loads(p.read_text())
def main():
    assert not OUT.exists();OUT.mkdir()
    manifest={'schema':'clef.positional-resident-upload-publication.v1','files':[],'split_files':[],
        'scope':'Positional resident-f16 upload correctness and 52 GiB qualification; no latency claim or independent-reference acceptance change.',
        'path_tokens':{token:meaning for (_,token),meaning in zip(pub.REPLACEMENTS,['Worktree root','Task scratch root','Pinned Python environment','Persistent Cargo target','Task session scratch root, including older sibling evidence','User home','Owned lease session identifier'])},
        'excluded':['model weight payloads','binary executables','raw pooled f16 tensors'],'raw_memory_scope':'Only task-owned process-group samples and aggregate host telemetry; no unrelated process tables.'}
    def save(name,raw):
        assert len(raw)<1_000_000 and (b'/'+b'Users/') not in raw,name
        p=OUT/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(raw)
    def ordinary(path,name,expected=None):
        raw=path.read_bytes()
        if expected:assert pub.sha(raw)==expected,path
        clean=pub.sanitize(raw)
        info={'path':name,'original_bytes':len(raw),'original_sha256':pub.sha(raw),'published_bytes':len(clean),'published_sha256':pub.sha(clean),'path_reconstruction_verified':True}
        if len(clean)<1_000_000:
            save(name,clean);manifest['files'].append(info);return
        parts=[];offset=0
        while offset<len(clean):
            end=min(offset+LIMIT,len(clean))
            while True:
                try:clean[offset:end].decode('utf-8');break
                except UnicodeDecodeError:end-=1
            name_part=name+'.parts/'+f'{len(parts):04d}.txt';part=clean[offset:end];save(name_part,part)
            parts.append({'path':name_part,'bytes':len(part),'sha256':pub.sha(part)});offset=end
        restored=b''.join((OUT/p['path']).read_bytes() for p in parts)
        assert restored==clean
        original=restored.decode()
        for old,token in reversed(pub.REPLACEMENTS):original=original.replace(token,old)
        assert original.encode()==raw
        info.update(parts=parts,reconstruction='Concatenate listed UTF-8 part bytes in order, without separators; then reverse the documented path tokens.',sanitized_and_original_reconstruction_verified=True)
        index=name+'.parts/index.json';save(index,pub.encoded(info));manifest['split_files'].append({'index':index,'index_sha256':pub.sha((OUT/index).read_bytes()),'original_sha256':pub.sha(raw),'original_bytes':len(raw)})
    build=load(HERE/'build-1/guard.json');fixture=load(HERE/'gpu-fixture-1/guard.json');text=load(HERE/'text-1/summary.json');images=load(HERE/'images-repair-v1/execution.json');image_run=load(HERE/'images-2/run.json');failed_images=load(HERE/'images-1/summary.json')
    records=[json.loads(line) for line in (HERE/'text-1/records.jsonl').read_text().splitlines()];shutdown=next(r for r in records if r.get('kind')=='shutdown')
    for record in (build,fixture,image_run):assert record['status'] in ('complete','failed') and record['memory']['owned_group_stopped']
    assert shutdown['owned_group_stopped'] and text['status'] in ('complete','failed')
    source=load(HERE/'qualification-source.json')
    for filename,digest in source['files'].items():
        path=Path(filename)
        name=('source/'+str(path.relative_to(HERE))) if path.is_relative_to(HERE) else 'source/dependencies/'+path.name
        ordinary(path,name,digest)
    ordinary(HERE/'qualification-source.json','qualification-source.json')
    for p in sorted(HERE.glob('*-prepared-v*.json')):ordinary(p,'preparation/'+p.name)
    refusal=load(HERE/'images-repair-v1/pre-execution-disk-refusal.json')
    assert not refusal['runner_started'] and not refusal['model_started']
    for p in sorted(HERE.glob('*-lease.json')):
        lease=load(p)
        assert lease['released_at_ms'],p
        if 'exit' not in lease:
            assert p.name=='images2-memory-lease.json' and refusal['status']=='refused_before_execution',p
        else:assert lease['exit'] in (0,1),p
        ordinary(p,'leases/'+p.name)
    ordinary(HERE/'negative-checks.json','preparation/negative-checks.json')
    ordinary(BASE/'rejected-q6-scratch-cleanup-v1/cleanup.json','addenda/rejected-q6-scratch-cleanup-v1/cleanup.json')
    ordinary(HERE/'candidate.json','candidate.json')
    for directory in ('build-1','gpu-fixture-1','text-1','images-1','images-2','images-repair-v1'):
        for path in sorted((HERE/directory).rglob('*')):
            if path.is_file() and path.suffix in ('.json','.jsonl','.log','.py','.md'):ordinary(path,str(path.relative_to(HERE)))
    patchdir=BASE/'resident-positional-upload-patch-v1';patch=load(patchdir/'frozen-sources.json')
    for name in ('frozen-sources.json','changes.patch'):ordinary(patchdir/name,'product-patch/'+name)
    for path,identity in patch['sources'].items():ordinary(ROOT/path,'product-source/'+path,identity['sha256'])
    old=BASE/'attention-qk128-key32-integration'
    for name in ('source-manifest.json','binaries.json','guard.json','27b-text-check/summary.json','27b-text-check/records.jsonl','27b-text-check/candidate/rows.jsonl','27b-image-check/run.json','27b-image-check/0-images.json'):
        ordinary(old/name,'baseline/'+name)
    inputs=load(HERE/'inputs.json')
    ordinary(BASE/'reference27b/images-mps-bf16.json','inputs/official-image-reference.json',inputs['files'][str(BASE/'reference27b/images-mps-bf16.json')])
    cases=ROOT/'crates/psionic/fixtures/clef/images/cases.json';ordinary(cases,'inputs/image-cases.json',inputs['files'][str(cases)])
    ordinary(ROOT/'crates/psionic/fixtures/clef/e2e/requests.jsonl','inputs/text-corpus.jsonl',inputs['files'][str(ROOT/'crates/psionic/fixtures/clef/e2e/requests.jsonl')])
    for filename,digest in inputs['files'].items():
        path=Path(filename)
        if path.is_relative_to(old) and (path.name.endswith('.request.json') or path.name.endswith('.response.json')):
            ordinary(path,'baseline/'+str(path.relative_to(old)),digest)
    image_asset_hashes={name:digest for name,digest in inputs['files'].items() if name.endswith(('.png','.jpg','.webp'))}
    save('inputs/image-assets.json',pub.sanitize(pub.encoded({'scope':'Existing checked-in image fixtures; payloads are not duplicated here.','files':image_asset_hashes})))
    summary={'status':'complete' if all(r['status']=='complete' for r in (build,fixture,text,images)) else 'failed',
        'build':{'seconds':build['seconds'],'cpu_passed':sum(r.get('passed',0) for r in load(HERE/'build-1/checks.json')),'memory':build['memory'],'disk_after_cleanup':build['disk_after_cleanup']},
        'gpu_fixture':{'tests_passed':fixture['tests_passed'],'seconds':fixture['seconds'],'memory':fixture['memory'],'peak_limit':'The two half-second samples bracket a 0.30-second test; the sampled peak is not a credible actual peak.'},
        'text':{'passed':text['passed'],'requests':text['comparison']['requests'],'question_vectors':text['comparison']['questions'],
                'scalar_logits':sum(len(r['candidate_logits']) for r in text['comparison']['comparisons']),'every_scalar_bit_equal':all(r['logit_bits_equal'] for r in text['comparison']['comparisons']),
                'every_pooled_byte_equal':all(r['pooled_bytes_equal'] for r in text['comparison']['comparisons']),'memory':shutdown},
        'images':images,'first_image_attempt':failed_images,'pre_execution_disk_refusal':refusal,'scratch_cleanup_addendum':load(BASE/'rejected-q6-scratch-cleanup-v1/cleanup.json'),'image_memory':image_run['memory'],'candidate':load(HERE/'candidate.json'),
        'unchanged_acceptance':'The independent Torch same-Q4 103/104 top-answer result remains unresolved. This is exact placement/upload equivalence against the native baseline.'}
    save('summary.json',pub.sanitize(pub.encoded(summary)))
    for name in ('pack.py','reconstruct.py'):ordinary(HERE/'publication'/name,'publication/'+name)
    readme=f'''# Positional resident upload qualification

The candidate keeps the same Q4 GGUF and external head, and changes native weight placement to resident f16 plus positional reads from the original open file. The saved native baseline used Q4-on-demand. Product source changes are limited to the five reviewed storage/upload/test files; encoder, head, and backend arithmetic sources are unchanged.

The build and 71 targeted CPU tests passed. The small GPU fixture passed exactly once. Its two half-second samples bracket a 0.30-second test, so its recorded sampled peak is not a credible actual peak.

Text qualification covers 42 requests: the unchanged 40-record/104-question corpus and fixed 4k/16k requests with 6 more questions. All 110 question vectors, containing 389 scalar f32 logits, and every indexed pooled f16 byte match exactly. This is 110 question vectors, not 110 scalar logits. Tokens, answer identities/tops, model metadata, zero cache, and complete export layout also passed.

Corrected image status: {images['status']}. The existing 6-image/14-answer and 10-refusal gate is unchanged: top answers, exact token counts, expected image contents, and refusal behavior. Image probability differences remain diagnostic. Full outcomes, including any failures, remain in the receipts.

Text sampled peak: {shutdown['peak_accounted_bytes']:,} bytes. Image sampled peak: {image_run['memory']['peak_accounted_bytes']:,} bytes. Both model attempts used 52 GiB max(RSS,physical footprint), 64 GiB available at startup, 12 GiB available floor, 600-second startup, 300-second request, and 1800-second server bounds. They are correctness runs with output export, not latency measurements. The limits are sampled guards, not hard allocation reservations.

The build used 24 GiB/start36/floor12/1800s, two jobs, Rust 1.97.1, and the persistent target-agent1 directory. Disk declarations were build 2 plus disk 4, with 31 GiB free at start and 25 GiB throughout/after cleanup. New qualification output stayed below 2 GiB. The GPU fixture used 4 GiB/start16/floor12/180s. The first image wrapper failed after its hardware checks completed because Python selected the existing shared runner instead of the intended frozen local helper. It lacked the extra source/disk checks and freshly hashed the GGUF. Its historical source identity and all failed receipts remain. A separately reviewed wrapper fixes only that import by loading the original local file explicitly, without changing model operators, input data, gates, or bounds. The corrected attempt uses `images-2`; the failed `images-1` is not overwritten. A disk refusal canceled an earlier queued attempt before the repaired wrapper or model started; both acquired leases were released, and no disk lease was acquired. The memory lease receipt has no child exit field because cancellation interrupted its outer lease chain. The coordinator then removed only the reproducible rejected Q6-down artifact (19,967,533,120 bytes), after checking its identity, references, and open-file users. The separate cleanup addendum preserves its generation/quality receipt hashes and 97/104 result; all source artifacts, numerical arrays, logs, and recipes remain. The corrected run used new lease receipts after free space recovered. There was no rebuild or repeated text run, and no raised bound. No quiet lease, desktop app, or system service was used or controlled. Cleanup covers only each owned process group.

The shutdown receipts retain any signal-zero permission observations after the owned leader exited; the guard independently verified zero remaining group members and recorded no cleanup error. Source and binary identities were checked again after the successful frozen build, fixture, text, and corrected-image runs. The failed first image wrapper did not execute its intended post-source check; this limitation remains explicit.

This result supports positional resident upload equivalence and the measured 52 GiB capacity for these cases. It does not resolve the independent Torch same-Q4 all-top acceptance discrepancy, replace that reference, improve the Q4 artifact's BF16 quality, or establish a speedup.

## Complete evidence

Sources, exact request bodies/responses, all comparisons, baseline metadata, source/input manifests, leases, guards, and owned memory samples are retained. Frozen preparation descriptors are labeled unexecuted. Model weights, binaries, and pooled f16 arrays are excluded; their hashes and comparison results remain. Large UTF-8 evidence files are split without losing bytes. Each ordered index records original/sanitized hashes and verified reconstruction; the reconstruction helper checks every part. Path substitutions are reversible with the private local map, which is kept outside this publication. No unrelated process tables are included.
'''
    save('README.md',readme.encode())
    for name in ('summary.json','README.md','inputs/image-assets.json'):
        raw=(OUT/name).read_bytes();manifest['files'].append({'path':name,'published_bytes':len(raw),'published_sha256':pub.sha(raw),'generated_from_saved_receipts':True})
    manifest['supersedes_publication']={'version':'publishable-v1','manifest_sha256':pub.sha((HERE/'publishable-v1/manifest.json').read_bytes()),'change':'Reversible sanitization now covers older sibling scratch paths and the owned lease session identifier; original evidence and numerical claims are unchanged.'}
    save('manifest.json',pub.encoded(manifest))
    (HERE/'publication/path-map.private.json').write_bytes(pub.encoded({token:old for old,token in pub.REPLACEMENTS}))
    for row in manifest['files']:assert pub.sha((OUT/row['path']).read_bytes())==row['published_sha256']
    print(json.dumps({'output':str(OUT),'files':sum(p.is_file() for p in OUT.rglob('*')),'bytes':sum(p.stat().st_size for p in OUT.rglob('*') if p.is_file()),'manifest_sha256':pub.sha((OUT/'manifest.json').read_bytes()),'split_files':len(manifest['split_files']),'status':summary['status']},indent=2))
if __name__=='__main__':main()
