"""Run the existing six-image/14-answer and ten-refusal acceptance unchanged."""
import json,os,sys
from pathlib import Path
import compare_attention as attention
from acceptance_helpers import bench
import native_image_parity
root=Path(__file__).resolve().parent
config=json.loads((root/'27b-image-config.json').read_text())
leases=set(os.environ.get('OPENAGENTS_LEASES','').split(','))
attention.require({'gpu','memory'}<=leases and 'quiet' not in leases,'requires GPU/memory without quiet')
attention.require(config['max_rss_gib']==52 and config['minimum_start_available_gib']==64 and config['minimum_available_gib']==12,'image memory bounds changed')
attention.require(config['startup_timeout']==600 and config['request_timeout']==300 and config['deadline_seconds']==1800,'image time bounds changed')
attention.require(config['env']==dict((k,v) for k,v in attention.ENV.items() if k.startswith('PSIONIC_'))|{'PSIONIC_CLEF_METAL_FFN_WEIGHTS':'q4k-on-demand'},'image mode flags differ')
attention.require(attention.load(root/'guard.json')['status']=='complete','candidate build incomplete')
attention.same_hash(config['binary'],attention.load(root/'binaries.json')['server']['sha256'])
receipts=attention.load(root/'baseline-receipts.json')['corpora']['27b']['artifacts']
attention.same_hash(config['gguf'],receipts['model_path']['sha256']);attention.same_hash(config['head'],receipts['clef_head']['sha256'])
old=attention.load(root.parent/'native27b-stage6-fused-images/run.json')
attention.same_hash(config['vision_manifest'],old['artifacts']['vision_manifest']['sha256'])
code=native_image_parity.run(config)
out=Path(config['output']);result=attention.load(out/'0-images.json') if (out/'0-images.json').exists() else {}
proof={'passed':False,'runner_exit':code,'source_sha256':{n:bench.file_sha(root/n) for n in ['check_images.py','native_image_parity.py']},'candidate_binary_sha256':bench.file_sha(config['binary'])}
try:
 attention.require(code==0 and result.get('passed') is True,'image acceptance failed')
 attention.require(len(result['cases'])==6 and sum(len(c['response']['answers']) for c in result['cases'])==14,'image coverage differs')
 attention.require(len(result['http_refusals'])==10 and all(r.get('passed') is True for r in result['http_refusals']),'refusal coverage differs')
 proof['passed']=True
except (ValueError,KeyError) as e:proof['error']=str(e)
attention.write(out/'coverage.json',proof)
raise SystemExit(0 if proof['passed'] else 1)
