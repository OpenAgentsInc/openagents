#!/usr/bin/env python3
"""Independent authored byte patterns and rounded-f32 filter expectations.

Verification tooling only. The production shader never reads these expected
values and uses the actual checked Coax filter helper.
"""
import hashlib,json,math,struct,sys
from pathlib import Path

def f(x):return struct.unpack('<f',struct.pack('<f',x))[0]
def mix(a,b,t):return [f(x+f(f(y-x)*t))for x,y in zip(a,b)]
def byte(x,y,level):return [(17*x+3*y+29*level)%256,(5*x+19*y+41*level)%256,(11*x+7*y+53*level)%256,(3*x+5*y+11*level)%256]
def sample_level(u,v,level):
 w,h=1536>>level,672>>level;fx=f(f(min(1,max(0,u))*w)-.5);fy=f(f(min(1,max(0,v))*h)-.5);x,y=math.floor(fx),math.floor(fy);tx,ty=f(fx-x),f(fy-y)
 def texel(ix,iy):return [f(c/255)for c in byte(min(w-1,max(0,ix)),min(h-1,max(0,iy)),level)]
 return mix(mix(texel(x,y),texel(x+1,y),tx),mix(texel(x,y+1),texel(x+1,y+1),tx),ty)
def expected(c):
 u,v,lod=map(f,[c['x'],c['y'],c['lod']]);lod=min(4,max(0,lod));low=math.floor(lod);high=min(4,low+1)
 return mix(sample_level(u,v,low),sample_level(u,v,high),f(lod-low))
def main():
 out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True);data=bytearray();levels=[]
 for level in range(5):
  w,h=1536>>level,672>>level;offset=len(data)
  for y in range(h):
   for x in range(w):data.extend(byte(x,y,level))
  levels.append(dict(width=w,height=h,offset=offset,row_bytes=w*4))
 raw=out/'canonical.bin';raw.write_bytes(data);bad=out/'bad-size.bin';bad.write_bytes(bytes(16))
 uv=[(-1,1.1,0),(0,0,0),(1,1,0),(.5/1536,.5/672,0),(1.5/1536,1.5/672,0),(.3,.35,0),(.3,.35,.1),(.3,.35,.2),(.3,.35,1/3),(.3,.35,.5),(.3,.35,1.25),(.3,.35,2.875),(.3,.35,4),(.3,.35,8),(.3,.35,-1),(.999999,.000001,3.5),(.5,.5,2.5),(.314159,.271828,3.125),(.0004,.0007,4),(.875,.75,1/1024)]
 cases=[dict(x=x,y=y,lod=z)for x,y,z in uv]
 textures=[dict(format='rgba8unorm',width=1536,height=672,file=str(raw.resolve()),sha256=hashlib.sha256(data).hexdigest(),levels=levels),dict(format='rgba8unorm',width=2,height=2,file=str(bad.resolve()),sha256=hashlib.sha256(bad.read_bytes()).hexdigest(),levels=[dict(width=2,height=2,offset=0,row_bytes=8)])]
 fixture=dict(schema=1,note='Independent byte-pattern fixture; second texture proves wrong canonical dimensions demote before any publication.',pixel=[9,9],clear=[.125,.25,.5,.75],cases=cases,textures=textures,expected=[expected(c)for c in cases]+[[.125,.25,.5,.75]for c in cases])
 (out/'cases.json').write_text(json.dumps(fixture,indent=2)+'\n')
if __name__=='__main__':main()
