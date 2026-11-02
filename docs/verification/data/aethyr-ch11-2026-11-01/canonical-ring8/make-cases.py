from pathlib import Path
import decimal,json,struct
D=decimal.Decimal;decimal.getcontext().prec=80
f=lambda x:struct.unpack('<f',struct.pack('<f',float(x)))[0]
h=f(D('0.5').sqrt());by=f('0.99999');assert struct.unpack('<I',struct.pack('<f',h))[0]==0x3f3504f3

def azimuth(t):
 if t>8:return None
 v=(1.,0.)if t%2==0 else(h,h)
 for i in range((t//2)%4):v=(-v[1],v[0])
 return v

def polar(r):
 if r>5:return None
 n=[-2,-1,0,0,1,2][r]
 return (0.,float(n//2))if abs(n)==2 else(1.,0.)if n==0 else(h,n*h)
rows=[];corners=[(0,0),(1,0),(1,1),(0,0),(1,1),(0,1)]
for vertex in range(240):
 q=vertex//6;cx,cy=corners[vertex%6];r=q//8+cy;a=azimuth(q%8+cx);p=polar(r);normal=[f(-a[1]*p[0]),p[1],f(-a[0]*p[0])];point=[normal[0],f((by if r>2 else 0)+normal[1]),normal[2]]
 for selector,value in enumerate([point+[1],normal+[1],list(a+p)]):rows.append({'vertex':vertex,'selector':selector,'uniform':[vertex,selector,0,0],'expected':value})
for selector,samples,fn in [(3,[0,8,9,65536],azimuth),(4,[0,5,6,65536],polar)]:
 for sample in samples:
  value=fn(sample);rows.append({'sample':sample,'selector':selector,'uniform':[sample,selector,0,0],'expected':[1,*value,0]if value is not None else[0,0,0,0]})
q={'schema':'aethyr.render.ring8-primitive-cases.v1','scope':'Independent analytic cardinal/diagonal directions: nearest-f32 sqrt(1/2) from80-digit Decimal, quarter rotations, exact poles, vertical unit-radius bone. No GPU output or old image is used to choose expected values.','width':8,'height':8,'pixel':[1,1],'clear':[-2,-2,-2,-2],'geometry_cases':720,'total_cases':len(rows),'comparison':'Every expected nonzero f32 word must match exactly; signed zeros compare numerically. Exact seam/pole equalities are separately checked within actual GPU outputs.','cases':rows};Path('/tmp/aethyr-ch11-ring8/cases.json').write_text(json.dumps(q,indent=2)+'\n');print('cases',len(rows))
