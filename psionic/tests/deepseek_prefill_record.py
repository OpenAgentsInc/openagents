#!/usr/bin/env python3
"""Replay the current-source original grouped MoE evidence without weights."""
from pathlib import Path
import gzip,hashlib,json
R=Path(__file__).resolve().parents[2];D=R/'psionic/fixtures/deepseek/prefill-original'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
j=json.loads((D/'validation.json').read_text());assert j['schema']==1
for name,digest in j['inputs'].items():assert sha(R/name)==digest,name
for name,digest in j['files'].items():assert sha(D/name)==digest,name
reference=R/j['reference'];assert sha(reference)==j['reference_sha256'] and sha(R/j['input'])==j['input_sha256']
raw=gzip.decompress((D/'output.f32.gz').read_bytes());ref=gzip.decompress(reference.read_bytes());assert len(ref)==20480 and raw==ref*4
log=(D/'run.log').read_text();assert log.count('staged once for 4 routed rows')==6 and 'all 20480 output words bit-identical' in log
assert '247726080 coefficient decodes versus 990904320 serial' in log
print('Prefill: current-source original 384/6 plus shared grouped MoE replay; 20480 exact words and six staged expert packets for 24 routed uses')
