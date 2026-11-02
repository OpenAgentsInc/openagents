from pathlib import Path
from fractions import Fraction as Q
import json,struct,math,hashlib,argparse

def sub(a,b):return tuple(x-y for x,y in zip(a,b))
def add(a,b):return tuple(x+y for x,y in zip(a,b))
def mul(a,s):return tuple(x*s for x in a)
def dot(a,b):return sum(x*y for x,y in zip(a,b))
def cross(a,b):return(a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0])
def normal(t):return cross(sub(t[1],t[0]),sub(t[2],t[0]))
def inside(p,t):
 n=normal(t);assert dot(n,n)>0
 return all(dot(cross(sub(t[(i+1)%3],t[i]),sub(p,t[i])),n)>=0 for i in range(3))
def intersections(a,b):
 na,nb=normal(a),normal(b);assert dot(na,na)>0 and dot(nb,nb)>0
 coplanar=all(dot(na,sub(v,a[0]))==0 for v in b)
 if coplanar:return True,None
 out=[]
 for t,u in [(a,b),(b,a)]:
  n=normal(u)
  for i in range(3):
   p,q=t[i],t[(i+1)%3];dp,dq=dot(n,sub(p,u[0])),dot(n,sub(q,u[0]))
   points=[]
   if dp==0:points.append(p)
   if dq==0:points.append(q)
   if dp*dq<0:points.append(add(p,mul(sub(q,p),dp/(dp-dq))))
   for p in points:
    if inside(p,a)and inside(p,b)and p not in out:out.append(p)
 return False,out
def tri(rows):return [tuple(Q(x)for x in row)for row in rows]
base=tri([(0,0,0),(1,0,0),(0,1,0)])
assert set(intersections(base,tri([(0,0,0),(0,1,0),(0,0,1)]))[1])=={(0,0,0),(0,1,0)}
assert intersections(base,tri([(0,0,1),(1,0,1),(0,1,1)]))==(False,[])
assert set(intersections(base,tri([(Q(1,4),Q(1,4),-1),(Q(1,4),Q(1,4),1),(Q(3,4),Q(1,4),0)]))[1])=={(Q(1,4),Q(1,4),0),(Q(3,4),Q(1,4),0)}

p=argparse.ArgumentParser();p.add_argument('--trace',type=Path,required=True);p.add_argument('--uniforms',type=Path,required=True);p.add_argument('--out',type=Path,required=True);a=p.parse_args()
trace=json.loads(a.trace.read_text());u=struct.unpack('<'+str(a.uniforms.stat().st_size//4)+'f',a.uniforms.read_bytes());eye=tuple(Q(x)for x in u[24:27])
q32=lambda w:Q(struct.unpack('<f',struct.pack('<I',w))[0])
project=lambda v:tuple(dot(tuple(Q(x)for x in u[8+i*4:12+i*4]),tuple(v)+(Q(1),))for i in range(4))
def screen(v):
 c=project(v);return((c[0]/c[3]+1)*160,(1-c[1]/c[3])*90)
def distance(p,a,b):
 v=sub(b,a);t=max(Q(0),min(Q(1),dot(sub(p,a),v)/dot(v,v)));d=sub(p,add(a,mul(v,t)));return math.sqrt(float(dot(d,d)))
pairs=[((42,76),(129,9),(129,27)),((124,118),(21,117),(23,69)),((67,122),(16,186),(17,138)),((138,134),(37,186),(38,138))]
rows=[]
for pixel,one,two in pairs:
 records=[];triangles=[]
 for inst,start in [one,two]:
  r=next(r for r in trace['fragment_records']if r['entry']==5 and(r['x'],r['y'])==pixel and r['instance']==inst and r['ids'][0]==start)
  t=[tuple(q32(w)for w in v['words'][:3])for v in r['vertices']];n=normal(t);triangles.append(t)
  authored=tuple(q32(w)for w in r['vertices'][0]['words'][3:6]);records.append({'instance':inst,'ids':r['ids'],'world':[[float(x)for x in v]for v in t],
   'world_words':[v['words'][:3]for v in r['vertices']],'geometric_normal_dot_authored':float(dot(n,authored)),
   'authored_normal_dot_eye':float(dot(authored,sub(eye,t[0]))),'geometric_normal_dot_eye':float(dot(n,sub(eye,t[0]))),
   'cpu_depth':r['depth'],'source_rgba':r['source']})
 coplanar,points=intersections(*triangles);na,nb=map(normal,triangles)
 cosine=float(dot(na,nb))/math.sqrt(float(dot(na,na)*dot(nb,nb)));angle=math.degrees(math.acos(max(-1,min(1,cosine))))
 record={'pixel':pixel,'contenders':records,'exact_binary32_world_coplanar':coplanar,'plane_angle_degrees':angle,'intersection_points':[[float(x)for x in v]for v in(points or[])],
  'shared_vertex_count':len(set(triangles[0])&set(triangles[1]))}
 if points and len(points)==2:
  segment=[screen(v)for v in points];record['ideal_real_transform_intersection_screen']=[[float(x)for x in v]for v in segment]
  record['pixel_centre_distance_to_ideal_projected_intersection']=distance((Q(pixel[0])+Q(1,2),Q(pixel[1])+Q(1,2)),*segment)
 rows.append(record)
report={'schema':'aethyr.render.deadgears-source-geometry-audit.v1','status':'diagnostic-only-not-a-numerical-oracle','scope':'Exact rational geometry from recorded binary32 world words. No GPU readback is consumed. Real-valued projection of those words is diagnostic only; it does not model f32VS/fixed-function arithmetic. Four requested contours are selected from the retained failure trace, not used as product admission or a mask.',
 'tool_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'trace':str(a.trace),'trace_sha256':hashlib.sha256(a.trace.read_bytes()).hexdigest(),'uniform_sha256':hashlib.sha256(a.uniforms.read_bytes()).hexdigest(),'independent_exact_geometry_controls':3,'rows':rows}
a.out.parent.mkdir(parents=True,exist_ok=True);a.out.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps([{k:r[k]for k in r if k!='contenders'}for r in rows],indent=2))
