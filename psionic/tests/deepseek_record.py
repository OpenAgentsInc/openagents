#!/usr/bin/env python3
"""Verify retained original numerical evidence with standard-library tooling."""
import array,gzip,hashlib,json,math,struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
D=ROOT/'psionic/fixtures/deepseek/cpu-original'
def sha(b):return hashlib.sha256(b).hexdigest()
def main():
 m=json.loads((D/'manifest.json').read_text())
 for n,x in m['files'].items():
  b=(D/n).read_bytes();assert len(b)==x['bytes'] and sha(b)==x['sha256'],n
 assert sha(gzip.decompress((D/'executed.c.gz').read_bytes()))==m['candidate']['generated_c_sha256']
 policy=(ROOT/'psionic/fixtures/deepseek/original/qualification-policy.json').read_bytes();report=json.loads((D/'comparison.json').read_text());assert report['policy_sha256']==sha(policy)
 p=json.loads(policy)
 for kind,n in [('logits',151936),('trace',86016)]:
  actual=array.array('f');actual.frombytes(gzip.decompress((D/f'prompt-{kind}.f32.gz').read_bytes()));reference=array.array('f');reference.frombytes(gzip.decompress((D/f'reference-{kind}.f32.gz').read_bytes()));assert len(actual)==len(reference)==n
  assert all(math.isfinite(a) and math.isfinite(b) and abs(a-b)<=p[kind+'_absolute']+p[kind+'_relative']*abs(b) for a,b in zip(actual,reference))
  if kind=='logits':assert max(range(n),key=actual.__getitem__)==max(range(n),key=reference.__getitem__)==71486
 query=array.array('f');query.frombytes(gzip.decompress((D/'slice-query.f32.gz').read_bytes()));expected_query=array.array('f');expected_query.frombytes(gzip.decompress((D/'slice-reference-query.f32.gz').read_bytes()));assert len(query)==len(expected_query)==1536
 assert all(math.isfinite(a) and abs(a-b)<=p['logits_absolute']+p['logits_relative']*abs(b) for a,b in zip(query,expected_query))
 slices=json.loads((D/'slices-build.json').read_text());assert sha(gzip.decompress((D/'slices-generated.c.gz').read_bytes()))==slices['generated_c_sha256']
 prompt=struct.unpack('<6q',(D/'prompt.i64').read_bytes());assert prompt==(151646,151644,13048,151645,151648,198)
 ids=struct.unpack('<64q',(D/'generated.i64').read_bytes());assert ids[-1]==151643 and ids.count(151649)==1
 assert (D/'final.txt').read_text()=='\n\nHello! How can I assist you today?'
 print('DeepSeek retained original: pinned build, all logits/layer values, greedy token, EOS and separate final verify')
if __name__=='__main__':main()
