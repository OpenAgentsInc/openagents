from pathlib import Path
import hashlib,json,os,signal,subprocess,sys,time,tarfile,difflib
ROOT=Path('/Users/christopherdavid/work/openagents-aethyr-book-main');sys.path.insert(0,str(ROOT/'aethyr/render/tests'))
from check_reference_frame import snapshot_frame_config
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
oldfile=Path('/tmp/aethyr-ch11-reference-icecream-bands-native-308/measurement.json');old=json.loads(oldfile.read_text())
assert old['status']=='rendered'and not any(old.get(n)for n in ['source_changes','compiler_changed','artifact_changes','input_changes'])
artifacts={kind:Path(old[kind+'_path'])for kind in ['generated_c','generated_header','executable']}
assert all(sha(p)==old[kind+'_sha256']for kind,p in artifacts.items())
current={n:sha(ROOT/n)for n in old['source_sha256']};deltas={n:{'historical':h,'current':current[n]}for n,h in old['source_sha256'].items()if h!=current[n]}
assert deltas=={'aethyr/render/geometry.coax':{'historical':'67fada3f27a8616b12716f3db9908f1994c6ae4dba2507fff798194c30f04318','current':'494bfde7afbdaaf741a67f9088cf108f59751fe01d39a770fcbd9a6ece507261'}}
archive=ROOT/'docs/verification/data/aethyr-ch11-2026-11-01/sans-owned-filter/frame-authority-633/source-closure.tar.gz'
with tarfile.open(archive)as t:oldgeometry=t.extractfile('aethyr/render/geometry.coax').read()
assert hashlib.sha256(oldgeometry).hexdigest()==deltas['aethyr/render/geometry.coax']['historical']
config=Path('/tmp/aethyr-ch11-icecream-doorway-main-half-320/frame.config');package=Path('/tmp/aethyr-ch11-packages-current/ice-cream-man/render.json');provenance=config.parent/'receipt.json'
started=time.monotonic();child=None
signal.signal(signal.SIGINT,lambda sig,frame:(_ for _ in()).throw(KeyboardInterrupt()))
signal.signal(signal.SIGTERM,lambda sig,frame:(_ for _ in()).throw(KeyboardInterrupt()))
for label,rounding in [('native','nearest-even'),('webgpu','nearest-positive')]:
 work=Path('/tmp/aethyr-ch11-reference-icecream-doorway-main-'+label+'-historical633');work.mkdir(exist_ok=False);inputs=work/'inputs';inputs.mkdir()
 q={'schema':'aethyr.render.cpu-frame-measurement.v1','status':'failed','qualification':False,'scope':'Fresh repaired actual producer frame through explicitly historical RF633 checked-O2 backend. Current-main Geometry differs only in Everglade spray capacity and tests; profile12 Ice has no water path. This is not a current-source CPU backend emission.','compiler_sha256':old['compiler_sha256'],'compiler_source_digest':old['compiler_source_digest'],'compiler_sources_match':False,'compiler_involved_in_this_execution':False,'native_mode':'checked-o2','c_mode':'checked','historical_backend_receipt':str(oldfile),'historical_backend_receipt_sha256':sha(oldfile),'source_sha256':old['source_sha256'],'current_source_observation_sha256':current,'historical_source_current_deltas':deltas,'commands':[]}
 try:
  files,package_copy,trees,capture_copy,counts=snapshot_frame_config(config,package,inputs,work/'frame.config')
  assert counts[0]==12 and counts[1:3]==[320,180]and counts[6]==0
  (work/'frame-provenance.json').write_bytes(provenance.read_bytes())
  (work/'geometry-historical.coax').write_bytes(oldgeometry);(work/'geometry-current.coax').write_bytes((ROOT/'aethyr/render/geometry.coax').read_bytes())
  (work/'geometry-delta.patch').write_text(''.join(difflib.unified_diff(oldgeometry.decode().splitlines(True),(ROOT/'aethyr/render/geometry.coax').read_text().splitlines(True),fromfile='RF633',tofile='current')))
  observed=set(files.values())|{work/'frame.config',capture_copy,package_copy,work/'frame-provenance.json'}|{p for _,p in trees};before={str(p):sha(p)for p in observed}
  image=work/'screen.rgba32f';result_path=work/'frame.json';args=[str(artifacts['executable']),str(work/'frame.config'),str(image),str(result_path),rounding,'8','viewport','0','rgba16f','complex-world','analytic'];begin=time.monotonic()
  with(work/'render.log').open('wb')as log:
   child=subprocess.Popen(args,cwd=ROOT,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
   try:code=child.wait(timeout=min(810,840-(time.monotonic()-started)))
   finally:
    if child.poll()is None:
     os.killpg(child.pid,signal.SIGKILL);child.wait()
    q['commands'].append({'args':args,'exit':child.returncode,'seconds':time.monotonic()-begin,'log_sha256':sha(work/'render.log')});child=None
  assert code==0
  result=json.loads(result_path.read_text());q.update(status='rendered'if result['status']==0 else'refused',result=result,render_seconds=time.monotonic()-begin,width=320,height=180,profile=12,slug='ice-cream-man',work_profile='complex-world',screen_format='rgba16f',filter_profile={'name':'analytic'},raster_profile={'rounding':rounding,'subpixel_bits':8,'xy_clipping':'viewport','guard_pixels':0},input_sha256=before,input_changes=[n for n,h in before.items()if sha(Path(n))!=h],source_changes=[n for n,h in current.items()if sha(ROOT/n)!=h],artifact_changes=[k for k,p in artifacts.items()if sha(p)!=old[k+'_sha256']],compiler_changed=False,screen_sha256=sha(image),screen_bytes=image.stat().st_size,frame_provenance_file=str(provenance),frame_provenance_sha256=sha(work/'frame-provenance.json'),package_sha256=sha(package_copy),capture_sha256=sha(capture_copy))
  for kind,p in artifacts.items():q[kind+'_path']=str(p);q[kind+'_sha256']=sha(p)
  assert not q['source_changes']and not q['input_changes']and not q['artifact_changes']
 except BaseException as error:q['error']=f'{type(error).__name__}:{error}';raise
 finally:
  q['elapsed_seconds']=time.monotonic()-started;(work/'measurement.json').write_text(json.dumps(q,indent=2)+'\n')
 print(json.dumps({'profile':label,'status':q['status'],'result':q.get('result'),'seconds':q.get('render_seconds')}),flush=True)
