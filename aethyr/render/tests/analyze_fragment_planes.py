#!/usr/bin/env python3
"""Original-plane reconstruction from observed generated-Coax clip outputs."""
import argparse,json,math,struct,hashlib
from pathlib import Path

def f32(n):return struct.unpack('<f',struct.pack('<f',n))[0]
def plane(row,width,height,mode):
 points=[]
 for v in row['vertices']:
  x,y,z,w=map(f32,v['clip'])
  if w<=0:raise ValueError('original homogeneous triangle crosses nonpositivew')
  x=(x/w+1)*width/2;y=(1-y/w)*height/2
  if mode=='even8':x=round(x*256)/256;y=round(y*256)/256
  if mode=='positive8':x=math.floor(x*256+.5)/256;y=math.floor(y*256+.5)/256
  points.append((x,y,z/w))
 (ax,ay,az),(bx,by,bz),(cx,cy,cz)=points;px=row['x']+.5;py=row['y']+.5
 den=(by-cy)*(ax-cx)+(cx-bx)*(ay-cy)
 if den==0:raise ValueError('degenerate original plane')
 a=((by-cy)*(px-cx)+(cx-bx)*(py-cy))/den;b=((cy-ay)*(px-cx)+(ax-cx)*(py-cy))/den
 return dict(depth=f32(a*az+b*bz+(1-a-b)*cz),screen_vertices=points,weights=[a,b,1-a-b])
def main():
 p=argparse.ArgumentParser();p.add_argument('fragments',type=Path);p.add_argument('output',type=Path);p.add_argument('--width',type=int,required=True);p.add_argument('--height',type=int,required=True);a=p.parse_args();source=json.loads(a.fragments.read_text());rows=[]
 for r in source['fragment_records']:
  if r['entry']!=5:continue
  rows.append(dict(x=r['x'],y=r['y'],instance=r['instance'],ids=r['ids'],observed_depth=r['depth'],written=r['written'],models={mode:plane(r,a.width,a.height,mode)for mode in ['unsnapped','even8','positive8']}))
 a.output.write_text(json.dumps(dict(schema='aethyr.render.original-fragment-planes.v1',scope='Independent diagnostic arithmetic from observed f32clip outputs, without altering the product renderer or claiming a device envelope.',source_sha256=hashlib.sha256(a.fragments.read_bytes()).hexdigest(),width=a.width,height=a.height,rows=rows),indent=2)+'\n')
 print(json.dumps([{k:r[k]for k in ['x','y','instance','ids','observed_depth']}|{'original_even8_depth':r['models']['even8']['depth']}for r in rows]))
if __name__=='__main__':main()
