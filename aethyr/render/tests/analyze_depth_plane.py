#!/usr/bin/env python3
# Firm reason: 2 (exact rational barycentric plane/depth and perspective interpolation, independent of the Coax rasterizer).
"""Independent exact-rational plane predictions for the authored GPU probe."""
import argparse,json,struct
from fractions import Fraction as F
from pathlib import Path

def f32(n):return struct.unpack('<f',struct.pack('<f',float(n)))[0]
def prediction(delta,perspective,mode,far_y=None,far_x=None):
 points=[(F(4)+F(delta),F(4)),(F(221,4),F(65,8)),(F(91,8),F(231,4))]
 if far_y is not None:points[2]=(points[2][0],F(far_y))
 if far_x is not None:points=[(F(40),F(8)+F(delta)),(F(40),F(56)),(F(far_x),F(32))]
 if mode!='unsnapped':
  def snap(v):
   q=v*256
   if mode=='even8':return F(round(q),256)
   q+=F(1,2);return F(q.numerator//q.denominator,256)
  points=[tuple(map(snap,p))for p in points]
 (ax,ay),(bx,by),(cx,cy)=points;px=py=F(41,2)
 determinant=(by-cy)*(ax-cx)+(cx-bx)*(ay-cy)
 a=((by-cy)*(px-cx)+(cx-bx)*(py-cy))/determinant
 b=((cy-ay)*(px-cx)+(ax-cx)*(py-cy))/determinant
 weights=[a,b,1-a-b];depths=[F(1,8),F(7,8),F(3,8)];values=[F(1,4),F(3,4),F(1,2)];w=[F(1),F(2),F(1,2)]if perspective else[F(1)]*3
 inv=sum(t/iw for t,iw in zip(weights,w));depth=sum(t*z for t,z in zip(weights,depths));varying=sum(t*v/iw for t,v,iw in zip(weights,values,w))/inv
 return[f32(depth),f32(varying),f32(inv),1.0]
def main():
 p=argparse.ArgumentParser();p.add_argument('gpu',type=Path);p.add_argument('output',type=Path);a=p.parse_args();gpu=json.loads(a.gpu.read_text());rows=[];maxima={m:[0.,0.,0.]for m in ['unsnapped','even8','positive8']}
 for r in gpu['results']:
  guesses={m:prediction(r['delta'],r['perspective'],m,r.get('farY'),r.get('farX'))for m in maxima}
  for mode,values in guesses.items():
   for k in range(3):maxima[mode][k]=max(maxima[mode][k],abs(values[k]-r['rgba'][k]))
  rows.append(dict(delta=r['delta'],perspective=r['perspective'],farY=r.get('farY'),farX=r.get('farX'),gpu=r['rgba'],predictions=guesses))
 record=dict(schema='aethyr.render.depth-plane-analysis.v1',scope='Exact rational analytic candidates from independent authored geometry; GPU precision observations, not a universal interpolation claim.',models_max_error_depth_varying_invw=maxima,rows=rows);a.output.write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(maxima))
if __name__=='__main__':main()
