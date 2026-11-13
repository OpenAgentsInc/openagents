# Firm reason: 6 (Blender's source .blend loader and evaluated mesh/skin API export the Source artwork).
"""Blender --background --disable-autoexec --python pack_fantasy_source.py -- SOURCE BASE_PACK OUTPUT
Append every individual Fantasy Source garment and all three base-color ways to the Metaverse Engine's existing
people pack, retaining its independently checked heads/hair and animation skeleton identity.
"""
import bpy,struct,sys,json,hashlib,math,tempfile
from pathlib import Path
from mathutils import Vector
source,base,output=map(Path,sys.argv[sys.argv.index('--')+1:]);folder=source/'Exports/glTF (Godot-Unreal)/Modular Parts'
raw=base.read_bytes();magic,n,total,budget,table,data_at,_,_=struct.unpack_from('<4s7I',raw)
assert magic==b'VPK1' and total==len(raw)
entries=[]
for i in range(n):
 kind,lic,_,offset,size,_,name=struct.unpack_from('<BBHIII32s',raw,table+i*48);assert offset+size<=len(raw);entries.append([kind,lic,name.rstrip(b'\0').decode(),raw[offset:offset+size]])
cat=entries[0][3].decode();vertices=bytearray(entries[1][3]);indices=bytearray(entries[2][3]);layers=sum(e[0]==4 for e in entries);lines=[];receipt={'source':'Quaternius Modular Character Outfits - Fantasy [Source]','license':'CC0 1.0 Universal','base_sha256':hashlib.sha256(raw).hexdigest(),'parts':[],'textures':[]}
# Native imported images are converted through Blender; no external imaging dependency.
family_layers={}
with tempfile.TemporaryDirectory() as temp:
 for family in ('Knight','Noble','Peasant','Ranger','Wizard'):
  family_layers[family]=layers
  for variant in range(3):
   name=f'T_{family}'+('' if variant==0 else f'_{variant+1}')+'_BaseColor.png';path=source/'Textures'/family/name
   im=bpy.data.images.load(str(path),check_existing=False);im.scale(512,512);dest=Path(temp)/name;im.filepath_raw=str(dest);im.file_format='PNG';im.save();content=dest.read_bytes();entries.append([4,4,f'layer{layers}',content]);lines.append(f'T {layers} {name} 4');receipt['textures'].append({'source':name,'source_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'layer':layers});layers+=1;bpy.data.images.remove(im)
 paths=[]
 for body in ('Female','Male'):
  path=source/f'All_{body}.blend';bpy.ops.wm.open_mainfile(filepath=str(path),load_ui=False,use_scripts=False)
  for arm in [o for o in bpy.data.objects if o.type=='ARMATURE']:arm.data.pose_position='REST'
  bpy.context.view_layer.update()
  garments=sorted([o for o in bpy.data.objects if o.type=='MESH' and o.name.startswith(body+'_') and 'Regular' not in o.name and len(o.vertex_groups)>0],key=lambda o:o.name)
  for obj in garments:
   paths.append(obj.name);arms=[m.object for m in obj.modifiers if m.type=='ARMATURE'];assert len(arms)==1
   reference=json.loads((folder/f'{body}_Peasant_Body.gltf').read_text());joints=[reference['nodes'][j]['name'] for j in reference['skins'][0]['joints']];assert len(joints)==65
   assert all(n in arms[0].data.bones for n in joints)
   family=obj.name.split('_')[1];start=len(indices)//4;word=len(vertices)//4;count=0;bounds=[];unique={}
   evaluated=obj.evaluated_get(bpy.context.evaluated_depsgraph_get());mesh=evaluated.to_mesh(preserve_all_data_layers=True,depsgraph=bpy.context.evaluated_depsgraph_get());mesh.calc_loop_triangles();world=obj.matrix_world;normal=world.to_3x3().inverted().transposed()
   uv=mesh.uv_layers.active;assert uv
   for tri in mesh.loop_triangles:
    for li in tri.loops:
     loop=mesh.loops[li];v=mesh.vertices[loop.vertex_index];p=world@v.co;nrm=(normal@mesh.corner_normals[li].vector).normalized();p=(p.x,p.z,-p.y);nrm=(nrm.x,nrm.z,-nrm.y);bounds.append(p)
     pairs=sorted([(g.weight,joints.index(obj.vertex_groups[g.group].name)) for g in v.groups if obj.vertex_groups[g.group].name in joints and g.weight>0],reverse=True)[:4];assert pairs
     sums=sum(w for w,j in pairs);ws=[int(w/sums*255) for w,j in pairs];ws[0]+=255-sum(ws);js=[j for w,j in pairs]
     while len(ws)<4:ws.append(0);js.append(0)
     # The imported evaluated surface supplies the canonical Metaverse Engine vertex encoder.
     uvco=uv.data[li].uv
     sys.path.insert(0,str(Path(__file__).resolve().parents[1]));import people
     record={'p':p,'n':nrm,'uv':(uvco.x,1-uvco.y),'j':js,'w':ws}
     encoded=people.vertex(record,family_layers[family],4 if family=='Ranger' else 3,(0,0,1,1),False)
     if encoded not in unique:unique[encoded]=count;vertices.extend(encoded);count+=1
     indices.extend(struct.pack('<I',unique[encoded]))
   evaluated.to_mesh_clear()
   assert count>0 and len(vertices)%4==0
   index_count=len(indices)//4-start
   name='fantasy/'+obj.name;lines.append(f'C {name} {start} {index_count} 4 {word} {count} 65')
   receipt['parts'].append({'name':obj.name,'source':path.name,'source_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'vertices':count,'triangles':index_count//3,'joint_names':joints});print('Packed',obj.name,index_count//3,flush=True)
 assert len(paths)==76,(len(paths),paths)
entries[1][3]=bytes(vertices);entries[2][3]=bytes(indices)
cat+='\nL 4 Quaternius Modular Character Outfits - Fantasy Source|CC0 1.0 Universal|https://quaternius.com/\n'+'\n'.join(lines)+'\n'
# Existing pack already has license 4; assign a fresh source license instead.
source_license=max(int(l.split()[1]) for l in entries[0][3].decode().splitlines() if l.startswith('L '))+1
cat=cat.replace('L 4 Quaternius Modular Character Outfits - Fantasy Source',f'L {source_license} Quaternius Modular Character Outfits - Fantasy Source')
for line in lines:
 fields=line.split()
 if fields[0]=='C':fields[4]=str(source_license)
 if fields[0]=='T':fields[-1]=str(source_license)
 cat=cat.replace(line,' '.join(fields))
for entry in entries:
 if entry[0]==4 and int(entry[2].removeprefix('layer'))>=min(family_layers.values()):entry[1]=source_license
limit=32*1024*1024
cat=__import__('re').sub(r'^budget \d+$',f'budget {limit}',cat,flags=__import__('re').M)
size=0
for _ in range(8):
 cat=__import__('re').sub(r'^bytes \d+$',f'bytes {size}',cat,flags=__import__('re').M);entries[0][3]=cat.encode();offset=32+48*len(entries);offsets=[]
 for e in entries:offsets.append(offset);offset+=(len(e[3])+3)&~3
 if size==offset:break
 size=offset
assert size<=limit and layers<=256
buf=bytearray(size);struct.pack_into('<4s7I',buf,0,b'VPK1',len(entries),size,limit,32,32+48*len(entries),0,0)
for i,(e,offset) in enumerate(zip(entries,offsets)):
 kind,lic,name,content=e;struct.pack_into('<BBHIII32s',buf,32+i*48,kind,lic,0,offset,len(content),0,name.encode());buf[offset:offset+len(content)]=content
output.parent.mkdir(parents=True,exist_ok=True);output.write_bytes(buf);receipt['pack_sha256']=hashlib.sha256(buf).hexdigest();receipt['pack_bytes']=len(buf);receipt['parts_count']=len(paths);receipt['texture_colorways']=3
output.with_suffix('.json').write_text(json.dumps(receipt,indent=2)+'\n');print('DONE',len(buf),'bytes',len(paths),'parts',layers,'layers')
