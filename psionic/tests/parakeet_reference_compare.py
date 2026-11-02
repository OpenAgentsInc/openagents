#!/usr/bin/env python3
# Firm reason: 2 (independent comparison of actual NeMo artifacts with the shipped Coax runtime).
"""Compare exact tokens/frame IDs and optionally complete feature/encoder tensors.

Reference is parakeet_reference.py's immutable output. Product dump prefix is
<id>.tokens.json, <id>.features.f32, <id>.encoded.f32, or id/tokens.json etc.
No model implementation or inference lives here. Coverage is explicit.
"""
from pathlib import Path
import argparse,array,hashlib,json,math,sys

def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def artifact(base,ident,name):
 a=base/ident/name
 if a.exists():return a
 return base/(ident+'.'+name)
def tensor(ref,got,absolute,relative):
 a=array.array('f');a.frombytes(ref.read_bytes());b=array.array('f');b.frombytes(got.read_bytes())
 if sys.byteorder!='little':a.byteswap();b.byteswap()
 if len(a)!=len(b):return {'passed':False,'reference_values':len(a),'product_values':len(b),'reason':'shape/byte extent differs'}
 worst=0.0;bad=0;index=-1;worst_ratio=0.0
 for i,(x,y) in enumerate(zip(a,b)):
  gap=abs(float(x)-float(y));limit=absolute+relative*abs(float(x))
  if not math.isfinite(x) or not math.isfinite(y) or gap>limit:bad+=1
  if gap>worst:worst=gap;index=i
  if math.isfinite(gap):worst_ratio=max(worst_ratio,gap/limit)
 return {'passed':bad==0,'values':len(a),'outside_tolerance':bad,'max_abs_gap':worst,'max_gap_index':index,'max_tolerance_ratio':worst_ratio,'abs_tolerance':absolute,'relative_tolerance':relative,'reference_sha256':sha(ref),'product_sha256':sha(got)}
def main():
 p=argparse.ArgumentParser();p.add_argument('reference',type=Path);p.add_argument('product',type=Path);p.add_argument('manifest',type=Path);p.add_argument('out',type=Path);p.add_argument('--decode-only',action='store_true');p.add_argument('--only',nargs='*');a=p.parse_args()
 reference=json.loads((a.reference/'record.json').read_text());manifest=json.loads(a.manifest.read_text());assert reference['passed'] and reference['manifest_unchanged'] and sha(a.manifest)==reference['fixture_manifest_sha256'];limits=manifest['acceptance'];rows=[];pins={}
 for clip in reference['clips']:
  ident=clip['id']
  if a.only and ident not in a.only:continue
  tokenfile=artifact(a.product,ident,'tokens.json');got=json.loads(tokenfile.read_text());pins[str(tokenfile)]=sha(tokenfile)
  row={'id':ident,'language':clip['language'],'corpus':clip['corpus'],'scope':'decoder-only on recorded NeMo encoder frames' if a.decode_only else 'complete features/encoder/decoder','tokens_equal':got['tokens']==clip['tokens'],'frames_equal':got['frames']==clip['frames'],'text_equal':got['text']==clip['text'],'reference_score':clip['score'],'reference_text':clip['text'],'product_text':got['text'],'product_tokens_sha256':sha(tokenfile)}
  if not a.decode_only:
   row['tensors']={}
   for name in ['features','encoded']:
    path=artifact(a.product,ident,name+'.f32');pins[str(path)]=sha(path);row['tensors'][name]=tensor(a.reference/clip['artifacts'][name]['file'],path,limits['tensor_tolerance_abs'],limits['tensor_tolerance_rel'])
  row['passed']=row['tokens_equal'] and row['frames_equal'] and row['text_equal'] and all(t['passed'] for t in row.get('tensors',{}).values());rows.append(row)
 assert rows and (not a.only or set(a.only)=={r['id'] for r in rows})
 record={'schema':'psionic.parakeet-nemo-comparison.v1','reference_record_sha256':sha(a.reference/'record.json'),'fixture_manifest_sha256':sha(a.manifest),'product_inputs':pins,'clips':rows,'passed':all(r['passed'] for r in rows),'full_inference_qualification':not a.decode_only,'scope':'Only named artifacts and this finite audio set; decoder-only comparisons do not qualify the product encoder.'};a.out.write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n');print(json.dumps({'passed':record['passed'],'clips':len(rows),'failed':[r['id'] for r in rows if not r['passed']]}));raise SystemExit(0 if record['passed'] else 1)
if __name__=='__main__':main()
