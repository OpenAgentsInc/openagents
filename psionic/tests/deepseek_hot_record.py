#!/usr/bin/env python3
"""Offline replay of the original cache-component evidence, without weights."""
from pathlib import Path
import gzip,hashlib,json
R=Path(__file__).resolve().parents[2];D=R/'psionic/fixtures/deepseek/hot-original'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
j=json.loads((D/'validation.json').read_text());assert j['schema']==1
for name,digest in j['inputs'].items():assert sha(R/name)==digest,name
for name,digest in j['files'].items():assert sha(D/name)==digest,name
reference=R/j['reference'];assert sha(reference)==j['reference_sha256']
actual=gzip.decompress((D/'output.f32.gz').read_bytes());expected=gzip.decompress(reference.read_bytes());assert len(actual)==20480 and actual==expected
log=(D/'run.log').read_text();assert all(f'mode {mode}, all 5120 output words bit-identical' in log for mode in range(3))
source=json.loads((D/'source.json').read_text());assert len(source['ranges'])==6 and sum(item['bytes'] for item in source['ranges'])==18800640
print('Hot experts: current-source original 5120x2304 host/pread/resident execution record, 5120 exact words and lossless packet provenance replay')
