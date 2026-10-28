"""Offline content conversion; never run by Xcode or shipped in the app.
Reads the reviewed scene capture and SCN4 asset bytes, uses official protoc
for the resource contract, and copies only referenced texture slices.
Run with the pinned protobuf tools venv: pack_haunt.py SOURCE_REPO OUTPUT.
"""
import hashlib, json, math, shutil, struct, subprocess, sys, tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
source,out=map(Path,sys.argv[1:]);out.mkdir(parents=True,exist_ok=True)
protoc=Path.home()/'.cache/openagents-engine-proto/protoc-29.3/bin/protoc'
tmp=tempfile.TemporaryDirectory(prefix='haunt-proto-')
subprocess.run([protoc,'-I'+str(ROOT/'proto'),'--python_out='+tmp.name,'openagents/metareal/haunt/v1alpha1/haunt.proto'],check=True)
sys.path.insert(0,tmp.name)
from openagents.metareal.haunt.v1alpha1.haunt_pb2 import Resource
cap=json.loads((ROOT/'verse/haunt/compiled.json').read_text());doc=json.loads((ROOT/'verse/haunt/house.json').read_text())
r=Resource(version=1,world_id='haunted-house',title=doc['title'],source_commit='69848ca')
def vec(to,values):to.x,to.y,to.z=values
scene=(source/'build-native/native-root/assets/scene.bin').read_bytes();n,nv,ni=struct.unpack_from('<III',scene,4);vb=16+n*100;ib=vb+nv*44
meshes={}
for k in range(n):
 at=16+k*100;name=scene[at:at+48].split(b'\0')[0].decode();first,count=struct.unpack_from('<II',scene,at+68)
 meshes[name]=[scene[vb+struct.unpack_from('<I',scene,ib+(first+i)*4)[0]*44:vb+struct.unpack_from('<I',scene,ib+(first+i)*4)[0]*44+44] for i in range(count)]
# Native engine's plain pitched roofs and gables, emitted as static content.
def lin(c):c/=255;return round((c/12.92 if c<=.04045 else ((c+.055)/1.055)**2.4)*255)
def prism(points,z0,z1,color):
 verts=[]
 def tri(a,b,c):
  u=[b[i]-a[i] for i in range(3)];v=[c[i]-a[i] for i in range(3)];nn=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];length=math.sqrt(sum(x*x for x in nn));normal=[x/length for x in nn]
  for p in [a,b,c]:verts.append(struct.pack('<8f12B',*p,*normal,.5,.5,*[lin(x) for x in color],27,*([0]*8)))
 for i in range(1,len(points)-1):
  tri((*points[0],z0),(*points[i+1],z0),(*points[i],z0));tri((*points[0],z1),(*points[i],z1),(*points[i+1],z1))
 for i,p in enumerate(points):
  q=points[(i+1)%len(points)];tri((*p,z0),(*q,z0),(*q,z1));tri((*p,z0),(*q,z1),(*p,z1))
 return verts
for w in [4,6,8]:
 h=w/2+.3;e=-.35;ridge=.55*w+.3;t=.2
 meshes[f'Plain_Roof_{w}']=prism([(-h,e),(-h,e+t),(0,ridge+t),(0,ridge)],-1,1,(176,84,60))+prism([(0,ridge),(0,ridge+t),(h,e+t),(h,e)],-1,1,(176,84,60))+prism([(-.12,ridge+t-.02),(0,ridge+t+.1),(.12,ridge+t-.02)],-1,1,(120,58,44))
 edge=max(0,e+(ridge-e)/h*.3)
 meshes[f'Plain_Gable_{w}']=prism([(-w/2,0),(w/2,0),(w/2,edge),(0,ridge-.02),(-w/2,edge)],-.1,.1,(222,204,178))
# Referenced art, plus the source's catalogue of addable props/creatures.
used={cap['meshes'][a['mesh']] for kind,a in cap['calls'] if kind in ('object-add','actor-figure')}
used|={t['mesh'] for t in cap['trees']}
used|={x for x in meshes if x.startswith(('Custom_','Potion_','CandleStick','Table_','Chair_','Bench','Barrel','Crate_','Bookcase_'))}
used=sorted(used);lookup={name:i for i,name in enumerate(used)};layers=sorted({v[35] for name in used for v in meshes[name]});r.texture_layers.extend(layers)
vertices=bytearray();indices=[]
for name in used:
 entries=meshes[name];m=r.meshes.add(name=name,first_index=len(indices),index_count=len(entries));unique={};points=[]
 for vertex in entries:
  if vertex not in unique:
   unique[vertex]=len(vertices)//48;points.append(struct.unpack_from('<3f',vertex));vertices+=vertex[:35]+bytes([layers.index(vertex[35])])+bytes(12)
  indices.append(unique[vertex])
 vec(m.minimum,[min(p[i] for p in points) for i in range(3)]);vec(m.maximum,[max(p[i] for p in points) for i in range(3)])
r.vertices_resource='Haunt.vertices';r.indices_resource='Haunt.indices';r.vertex_count=len(vertices)//48;r.index_count=len(indices)
(out/'Haunt.vertices').write_bytes(vertices);(out/'Haunt.indices').write_bytes(struct.pack('<%dI'%len(indices),*indices))
objects=[];lights=[];actors=[]
for kind,a in cap['calls']:
 if kind=='object-add':
  e=r.entities.add(mesh=lookup[cap['meshes'][a['mesh']]],yaw=a.get('yaw',0),scale=a.get('scale',1),glow=a.get('glow',0),fixed=True);vec(e.position,[a.get(k,0) for k in ['x','y','z']]);vec(e.tint,a.get('tint',[1,1,1]));objects.append(e)
 elif kind=='name-object':objects[a['id']].id=a['name']
 elif kind=='object-show':objects[a['id']].hidden=not a['on']
 elif kind=='object-motion':objects[a['id']].motion={'rocking':1,'ghost':2,'bubbling':3}[a['kind']]
 elif kind=='light-add':
  l=r.lights.add(radius=a['radius'],flicker=a.get('flicker',0),intensity=1);vec(l.position,[a.get(k,0) for k in ['x','y','z']]);vec(l.color,a['color']);lights.append(l)
 elif kind=='light-set':lights[a['id']].intensity=a.get('intensity',1)
 elif kind=='name-light':lights[a['id']].parent=a['name']
 elif kind=='walk-add':r.walk.add(**{k:a[k] for k in ['x0','z0','x1','z1']})
 elif kind=='block-add':r.blocks.add(**{k:a[k] for k in ['x0','z0','x1','z1']})
 elif kind=='actor-add':actors.append(a)
 elif kind=='actor-figure':
  p=actors[a['id']];original=doc['npcs'][a['id']];e=r.entities.add(id=original['id'],name=original['name'],mesh=lookup[cap['meshes'][a['mesh']]],yaw=p['yaw'],scale=1,solid=True,fixed=True);vec(e.position,[p['x'],0,p['z']]);vec(e.tint,[1,1,1])
for i,t in enumerate(cap['trees']):
 e=r.entities.add(id=f'tree-{i}',name='Tree',mesh=lookup[t['mesh']],yaw=t['yaw'],scale=t['scale'],motion=6,solid=t['collide'],fixed=True);vec(e.position,[t[k] for k in ['x','y','z']]);vec(e.tint,t['tint'])
for ob in doc['objects']:
 h=cap['handles']['objects'][ob['id']];eid=h.get('id',-1)
 if eid<0:continue
 e=objects[eid];e.name=ob['name'];e.fixed=ob.get('fixed',False);e.solid=ob.get('block',False)
 for child,fi in enumerate(h.get('flames',[])):objects[fi].id=e.id+f'/flame-{child}';objects[fi].motion=4;objects[fi].parent=e.id
for door in doc['doors']:
 eid=cap['handles']['doors'].get(door['id'],{}).get('id',-1)
 if eid>=0:objects[eid].motion=5;objects[eid].name='Door';objects[eid].solid=door.get('walk',True)==False
for i,e in enumerate(r.entities):
 if not e.id:e.id=f'structure-{i}'
for a in doc['areas']:
 x=r.areas.add(id=a['id'],name=a['name'],room=a.get('kind')=='room',dark=a.get('dark',False),surface={'wood':1,'grass':2,'metal':3}.get(a.get('surface','grass'),0));x.rect.x0,x.rect.z0,x.rect.x1,x.rect.z1=a['rect']
for p in doc['npcs']+[{**v,'id':k} for k,v in doc['voices'].items()]:
 if not p.get('id'):continue
 ob=next((o for o in doc['objects'] if o['id']==p['id']),p)
 x=r.people.add(id=p['id'],name=p['name'],role=p.get('role',''),knowledge=p.get('knowledge',''),range=3.2);vec(x.position,[ob.get('x',0),ob.get('y',0),ob.get('z',0)])
 for key in ['greet','hint','answer','emote']:getattr(x,key).extend(p.get('lines',p).get(key,[]))
for z in doc['zones']:
 t=z.get('Trigger',z.get('trigger',{}));pos=z.get('Transform',{}).get('pos',[z.get('x',0),z.get('y',0),z.get('z',0)]);x=r.zones.add(id=t['id'],name=t.get('name',''),radius=t.get('radius',0),sphere=t.get('shape')=='sphere',once=t.get('once',False));vec(x.position,pos);vec(x.half,t.get('half',[0,0,0]))
 for a in t.get('do',[]):
  q=x.actions.add(**{k:a[k] for k in ['op','target','name','text','value','duration','delay','flat','volume'] if k in a});vec(q.at,a.get('at',[0,0,0]));vec(q.size,a.get('size',[0,0,0]))
vec(r.arrival,[doc['arrival']['x'],0,doc['arrival']['z']]);r.arrival_yaw=doc['arrival']['yaw']*math.pi/180+math.pi
(out/'Haunt.pb').write_bytes(r.SerializeToString(deterministic=True))
for layer in layers:
 src=next((source/'assets'/f'layer{layer}.{ext}' for ext in ['png','jpg'] if (source/'assets'/f'layer{layer}.{ext}').exists()),None);assert src;shutil.copyfile(src,out/f'haunt-layer{layer}{src.suffix}')
for name in ['wind_loop','hiss','slam','thunder','creak','chime','bubble','boom','boo','jump','land','click']+[f'step_{surface}_{i}' for surface in ['stone','wood','grass','metal'] for i in [1,2,3]]:shutil.copyfile(source/'assets/audio'/f'{name}.mp3',out/f'haunt-{name}.mp3')
manifest={'source_commit':'69848ca','scene_sha256':hashlib.sha256(scene).hexdigest(),'resource_sha256':hashlib.sha256((out/'Haunt.pb').read_bytes()).hexdigest(),'entities':len(r.entities),'meshes':len(r.meshes),'areas':len(r.areas),'walk_rectangles':len(r.walk),'lights':len(r.lights),'people':len(r.people),'zones':len(r.zones),'texture_layers':layers,'attribution':'Quaternius CC0 modular dungeon/village kits; custom Haunted House art OpenAgents Inc.'}
(ROOT/'verse/haunt/assets.json').write_text(json.dumps(manifest,indent=2)+'\n');print(manifest)
