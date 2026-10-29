#!/usr/bin/env python3
"""Compare GPU captures with recorded independent original CPU answers.
No original CPU inference or NumPy runtime is needed for this check.
The bound is fixed before any GPU captures are read.
"""
import argparse,ast,hashlib,json,math,struct,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
BOUND=5e-4
def npy(blob):
    assert blob[:6]==b'\x93NUMPY';major=blob[6];width=2 if major==1 else 4;start=8+width;n=int.from_bytes(blob[8:start],'little');h=ast.literal_eval(blob[start:start+n].decode());assert not h['fortran_order'] and len(h['shape'])==1
    data=blob[start+n:];assert h['descr'] in ('<f4','<i8');assert len(data)==h['shape'][0]*(4 if h['descr']=='<f4' else 8);return data,h['descr']
def main():
    ap=argparse.ArgumentParser();ap.add_argument('model',choices=['20b','120b']);ap.add_argument('results',type=Path);a=ap.parse_args();archive=ROOT/'psionic/fixtures/gptoss/original'/a.model/'qualification/reference.npz';summary={'profile':'gpt-oss-cuda-f32-v1','reference_sha256':hashlib.sha256(archive.read_bytes()).hexdigest(),'bound':BOUND,'steps':[]}
    with zipfile.ZipFile(archive) as z:
        for step in range(2):
            r={}
            for field in ('logits','trace','routes','weights'):
                data,dtype=npy(z.read(f'{step}_{field}.npy'));path=a.results/f'step{step}.{field}.{"i64" if field=="routes" else "f32"}';got=path.read_bytes();assert len(got)==len(data)
                if dtype=='<i8':assert got==data,(step,field,'expert IDs differ');r[field]={'exact':True}
                else:
                    ref=[x[0] for x in struct.iter_unpack('<f',data)];values=[x[0] for x in struct.iter_unpack('<f',got)];assert all(map(math.isfinite,values));errors=[abs(x-y)/(1+abs(y)) for x,y in zip(values,ref)];largest=max(errors);assert largest<BOUND,(step,field,largest,BOUND);r[field]={'maximum_scaled_error':largest}
                    if field=='logits':
                        expected=max(range(len(ref)),key=lambda i:ref[i]);actual=max(range(len(values)),key=lambda i:values[i]);assert actual==expected,(step,'top token',actual,expected);r[field]['top_token']=actual
                r[field]['candidate_sha256']=hashlib.sha256(got).hexdigest()
            summary['steps'].append(r)
    (a.results/'qualification.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary,indent=2))
if __name__=='__main__':main()
