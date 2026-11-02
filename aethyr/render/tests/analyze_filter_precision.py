#!/usr/bin/env python3
# Firm reason: 2 (independent real-number interpolation and explicitly named quantization models for recorded GPU observations).
"""Fit declared numerical candidates to independent GPU filter observations.

This is diagnostic tooling. It neither changes a renderer profile nor grants a
numerical pass to an application image. Raw texels and uniforms come from the
authored fixture, and all candidate errors are retained.
"""
import argparse
import itertools
import json
import math
from pathlib import Path
import struct

def f32(x):
 return struct.unpack('<f',struct.pack('<f',x))[0]

def half(x):
 return struct.unpack('<e',struct.pack('<e',x))[0]

def snap(x,denominator,rounding='positive'):
 if denominator==0:return x
 if denominator==-1:return half(x)
 scaled=x*denominator
 value=math.floor(scaled) if rounding=='floor' else (math.floor(scaled+0.5) if rounding=='positive' else round(scaled))
 return value/denominator

def mix(a,b,t):return [a[k]+(b[k]-a[k])*t for k in range(4)]

def decode(texture):
 raw=bytes(texture['bytes'])
 if texture['format']=='rgba8unorm':return [[raw[k+c]/255 for c in range(4)]for k in range(0,len(raw),4)]
 if texture['format']=='rgba16float':return [list(struct.unpack_from('<4e',raw,k))for k in range(0,len(raw),8)]
 raise ValueError('unknown authored texture format')

def predict(texture,case,coefficient,denominator,stages,mip_coefficient,rounding='positive',mip_rounding='positive',joint=False,residual=False):
 texels=decode(texture)
 # Match only the shader's explicit f32 UV construction; interpolation models
 # themselves are independent real-number expressions followed by named rounds.
 u=f32(f32(f32(0.5)+f32(case['x']))*f32(0.5));v=f32(f32(f32(0.5)+f32(case['y']))*f32(0.5))
 tx=snap(f32(f32(u*2)-0.5),coefficient);ty=snap(f32(f32(v*2)-0.5),coefficient)
 quant=lambda c:[snap(x,denominator,rounding)for x in c]
 lod=snap(f32(case['lod']),mip_coefficient,mip_rounding)
 if joint:
  mass=1-lod
  if residual:
   right=snap(mass*tx,256);bottom=snap(mass*ty,256)
   left=mass-right;top=mass-bottom
   first=snap(mass*(1-tx)*(1-ty),256)
   first=max(max(0,left-bottom),min(min(left,top),first))
   weights=[first,top-first,left-first,mass-top-left+first]
  else:weights=[snap(mass*(1-tx)*(1-ty),256,'even'),snap(mass*tx*(1-ty),256,'even'),snap(mass*(1-tx)*ty,256,'even'),snap(mass*tx*ty,256,'even')]
  out=[sum(texels[i][k]*weights[i]for i in range(4))+texels[4][k]*lod for k in range(4)]
  if 'm'in stages:out=quant(out)
  return [f32(x)for x in out]
 a=mix(texels[0],texels[1],tx);b=mix(texels[2],texels[3],tx)
 if 'x'in stages:a=quant(a);b=quant(b)
 low=mix(a,b,ty)
 if 'y'in stages:low=quant(low)
 out=mix(low,texels[4],lod)
 if 'm'in stages:out=quant(out)
 return [f32(x)for x in out]

def candidates(fixture,results):
 textures={t['format']:t for t in fixture['textures']}
 if len(results)!=40:raise ValueError('expected all 40 independent observations')
 rows=[]
 for fmt in textures:
  selected=[r for r in results if r['format']==fmt]
  if len(selected)!=20:raise ValueError('missing format observations')
  for coefficient,denominator,stages,mip_coefficient,rounding,mip_rounding,joint,residual in itertools.product([0,256],[0,255,4080,65535,-1],['','m','ym','xym'],[0,64,256],['positive','even'],['positive','floor'],[False,True],[False,True]):
   if not joint and residual:continue
   errors=[]
   for expected,observed in zip(fixture['cases'],selected):
    if any(observed[k]!=expected[k]for k in ['x','y','lod']):raise ValueError('observation order/input mismatch')
    predicted=predict(textures[fmt],expected,coefficient,denominator,stages,mip_coefficient,rounding,mip_rounding,joint,residual)
    errors.extend(abs(x-y)for x,y in zip(predicted,observed['rgba']))
   rows.append(dict(format=fmt,coefficient=coefficient,result_denominator=denominator,round_stages=stages,mip_coefficient=mip_coefficient,rounding=rounding,mip_rounding=mip_rounding,joint=joint,residual=residual,max_error=max(errors),sum_error=sum(errors)))
 return sorted(rows,key=lambda r:(r['format'],r['max_error'],r['sum_error']))

def main():
 p=argparse.ArgumentParser();p.add_argument('fixture',type=Path);p.add_argument('observation',type=Path);p.add_argument('output',type=Path);a=p.parse_args()
 fixture=json.loads(a.fixture.read_text());observation=json.loads(a.observation.read_text())
 results=observation.get('results')
 if results is None:
  rgba=observation['rgba'];results=[dict(format=t['format'],**c,rgba=rgba[ti*20+i])for ti,t in enumerate(fixture['textures'])for i,c in enumerate(fixture['cases'])]
 rows=candidates(fixture,results)
 out=dict(scope='Independent filter candidate analysis; no product/image qualification.',candidates=rows)
 a.output.write_text(json.dumps(out,indent=2)+'\n')
 for fmt in [t['format']for t in fixture['textures']]:print(fmt,json.dumps([r for r in rows if r['format']==fmt][:8]))
if __name__=='__main__':main()
