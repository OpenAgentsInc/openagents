# Firm reason: 6 (Blender SDK evaluates authored garment surfaces, curves and source skin weights).
"""Blender --background --disable-autoexec --python pack_tideglass.py -- DESIGN.blend SOURCE_DIR PACK
Append the authored Tideglass outfit to the existing VPK1, using its existing 24-byte people ABI.
Source sleeve presentation rotations are undone before binding. New cape/details borrow nearest
source garment weights; the rigid mask binds to Head. Both body fits use their own rest skeleton.
"""
import bpy,sys,struct,json,math,hashlib,tempfile,re
from pathlib import Path
from mathutils import Vector,Matrix,kdtree
sys.path.insert(0,str(Path(__file__).resolve().parents[1]));import people
design,source,pack=map(Path,sys.argv[sys.argv.index('--')+1:])
bpy.ops.wm.open_mainfile(filepath=str(design),use_scripts=False,load_ui=False)
objects=[]
for ob in list(bpy.data.objects):
 if ob.type not in ('MESH','CURVE') or ob.name in ('Cylinder','Plane') or 'stage' in ob.name.lower():continue
 if ob.name.startswith('Redesigned Female_Wizard_Arms'):
  for v in ob.data.vertices:
   s=1 if v.co.x>0 else -1;pivot=Vector((s*.20,0,1.4));v.co=pivot+Matrix.Rotation(-s*math.radians(62),3,'Y')@(v.co-pivot)
 if 'gloves' in ob.name:continue # original sleeves retain the source hands
 bpy.context.view_layer.update();dg=bpy.context.evaluated_depsgraph_get();me=bpy.data.meshes.new_from_object(ob.evaluated_get(dg),depsgraph=dg);me.calc_loop_triangles()
 colors=[tuple(m.diffuse_color[:3]) if m else (.1,.1,.1) for m in me.materials]
 verts=[tuple(ob.matrix_world@v.co) for v in me.vertices];tris=[]
 for tri in me.loop_triangles:
  tris.append((tuple(tri.vertices),tuple(tuple((ob.matrix_world.to_3x3()@me.corner_normals[li].vector).normalized()) for li in tri.loops),colors[tri.material_index]))
 objects.append((ob.name,verts,tris))
raw=pack.read_bytes();_,n,_,budget,table,_,_,_=struct.unpack_from('<4s7I',raw);entries=[]
for i in range(n):
 kind,lic,_,off,size,_,name=struct.unpack_from('<BBHIII32s',raw,table+48*i);entries.append([kind,lic,name.rstrip(b'\0').decode(),raw[off:off+size]])
assert 'original/Female_Tideglass' not in entries[0][3].decode(),'Run against a fresh fantasy pack'
cat=entries[0][3].decode();vb=bytearray(entries[1][3]);ib=bytearray(entries[2][3]);layer=sum(e[0]==4 for e in entries);lic=max(int(l.split()[1]) for l in cat.splitlines() if l.startswith('L '))+1
colors=sorted(set(c for _,_,ts in objects for _,_,c in ts));assert len(colors)<64
# Palette texture retained as three identical ways, compatible with the existing variant ABI.
im=bpy.data.images.new('Tideglass authored palette',width=512,height=512);pixels=[]
for y in range(512):
 for x in range(512):pixels.extend((*colors[min(len(colors)-1,x*len(colors)//512)],1))
im.pixels[:]=pixels
with tempfile.TemporaryDirectory() as td:
 path=Path(td)/'palette.png';im.filepath_raw=str(path);im.file_format='PNG';im.save();png=path.read_bytes()
for v in range(3):entries.append([4,lic,f'layer{layer+v}',png]);cat+=f'\nT {layer+v} Tideglass_Palette_{v} {lic}\n'
receipt=[]
for sex in ('Female','Male'):
 bpy.ops.wm.open_mainfile(filepath=str(source/f'All_{sex}.blend'),use_scripts=False,load_ui=False)
 reference=json.loads((source/f'Exports/glTF (Godot-Unreal)/Modular Parts/{sex}_Peasant_Body.gltf').read_text());joints=[reference['nodes'][j]['name'] for j in reference['skins'][0]['joints']]
 samples=[]
 for ob in bpy.data.objects:
  if ob.type!='MESH' or not ob.name.startswith(sex+'_') or 'Regular' in ob.name:continue
  for v in ob.data.vertices:
   pairs=sorted([(g.weight,joints.index(ob.vertex_groups[g.group].name)) for g in v.groups if ob.vertex_groups[g.group].name in joints and g.weight>0],reverse=True)[:4]
   if pairs:samples.append((ob.matrix_world@v.co,pairs))
 tree=kdtree.KDTree(len(samples))
 for i,(p,_) in enumerate(samples):tree.insert(p,i)
 tree.balance();start=len(ib)//4;word=len(vb)//4;unique={}
 for name,vs,ts in objects:
  for ids,norms,c in ts:
   for vi,nrm in zip(ids,norms):
    p=Vector(vs[vi]);p.x*=1.055 if sex=='Male' else 1
    pairs=[(1,joints.index('Head'))] if 'mask' in name.lower() or 'sight slit' in name.lower() else samples[tree.find(p)[1]][1]
    weights=[int(w/sum(x[0] for x in pairs)*255) for w,j in pairs];weights[0]+=255-sum(weights);js=[j for w,j in pairs]
    while len(js)<4:js.append(0);weights.append(0)
    rec={'p':(p.x,p.z,-p.y),'n':(nrm[0],nrm[2],-nrm[1]),'uv':((colors.index(c)+.5)/len(colors),.5),'j':js,'w':weights}
    kind=5 if c[1]>.1 and c[0]<.03 and c[2]<.2 else 3 if min(c)>.35 else 0
    encoded=people.vertex(rec,layer,kind,(0,0,1,1),False)
    if encoded not in unique:unique[encoded]=len(unique);vb.extend(encoded)
    ib.extend(struct.pack('<I',unique[encoded]))
 count=len(ib)//4-start;cat+=f'\nC original/{sex}_Tideglass {start} {count} {lic} {word} {len(unique)} 65\n';receipt.append({'body':sex,'vertices':len(unique),'triangles':count//3});print(receipt[-1],flush=True)
cat+=f'\nL {lic} OpenAgents Tideglass Wayfarer, derived from Quaternius CC0 source|CC0 1.0 Universal|https://quaternius.com/\n';entries[1][3]=bytes(vb);entries[2][3]=bytes(ib)
size=0
for _ in range(10):
 cat=re.sub(r'^bytes \d+$',f'bytes {size}',cat,flags=re.M);entries[0][3]=cat.encode();offset=32+48*len(entries);offsets=[]
 for e in entries:offsets.append(offset);offset+=(len(e[3])+3)&~3
 if offset==size:break
 size=offset
assert size<=budget
buf=bytearray(size);struct.pack_into('<4s7I',buf,0,b'VPK1',len(entries),size,budget,32,32+48*len(entries),0,0)
for i,(e,off) in enumerate(zip(entries,offsets)):
 kind,l,name,data=e;struct.pack_into('<BBHIII32s',buf,32+48*i,kind,l,0,off,len(data),0,name.encode());buf[off:off+len(data)]=data
pack.write_bytes(buf);meta=json.loads(pack.with_suffix('.json').read_text());meta['original_outfit']={'name':'Tideglass Wayfarer','design_sha256':hashlib.sha256(design.read_bytes()).hexdigest(),'binding':'nearest source garment weights; rigid Head mask; male width fit 1.055','parts':receipt};meta['pack_sha256']=hashlib.sha256(buf).hexdigest();meta['pack_bytes']=size;pack.with_suffix('.json').write_text(json.dumps(meta,indent=2)+'\n');print('PACKED',size)
