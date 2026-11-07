# Firm reason: 2 (independent binary-layout/source-geometry oracle for the Coax glade compiler).
import hashlib, json, math, struct, sys, re, zipfile
from pathlib import Path

p = Path(sys.argv[1]); base = Path(sys.argv[2]); raw = p.read_bytes(); old = base.read_bytes()
repo = Path(__file__).resolve().parents[3]
revision = json.loads((repo/'assets/registry/scenes/character-glade.json').read_text())['reference']['revision']
experimental = '--experimental' in sys.argv[3:]
village = '--village' in sys.argv[3:]
props = '--props' in sys.argv[3:]
showcase = '--showcase' in sys.argv[3:]
animal_run = '--animal-run' in sys.argv[3:]
assert not props or village
assert not village or experimental
source = Path(sys.argv[3]) if len(sys.argv)>3 and sys.argv[3]!='--experimental' else Path.home()/'.openagents/assets/originals'/revision/'source'

def parts(b):
    magic, n, total, budget, table, *_ = struct.unpack_from('<4s7I', b)
    assert (magic==b'VPK1' or (magic==b'VPK2' and n>48)) and len(b)==total<=budget<=(128 if showcase else 64 if village else 32)*1024*1024
    out = {}
    for i in range(n):
        kind, license, _, offset, size, _, name = struct.unpack_from('<BBHIII32s', b, table+i*48)
        assert offset%4==0 and offset+size<=len(b)
        out[name.rstrip(b'\0').decode()] = (kind, b[offset:offset+size])
    return out

def accessor(doc, blob, index):
    a=doc['accessors'][index]; v=doc['bufferViews'][a['bufferView']]
    fmt={5121:'B',5123:'H',5125:'I',5126:'f'}[a['componentType']]
    width={'SCALAR':1,'VEC2':2,'VEC3':3,'VEC4':4}[a['type']]
    size=struct.calcsize('<'+fmt*width); stride=v.get('byteStride',size)
    start=v.get('byteOffset',0)+a.get('byteOffset',0)
    return [struct.unpack_from('<'+fmt*width,blob,start+i*stride) for i in range(a['count'])]

new=parts(raw); orig=parts(old)
cat=next(v[1].decode() for v in new.values() if v[0]==1)
assert int(next(l.split()[1] for l in cat.splitlines() if l.startswith('bytes ')))==len(raw)
row=next(l.split() for l in cat.splitlines() if l.startswith('C glade/environment '))
first,count,license,word,verts,joints=map(int,row[2:]); src=sum(1 for l in cat.splitlines() if l.startswith('L ')); assert joints==0 and license==(250 if experimental else src-1)
vb=next(v[1] for v in new.values() if v[0]==2); ib=next(v[1] for v in new.values() if v[0]==3)
oldvb=next(v[1] for v in orig.values() if v[0]==2); oldib=next(v[1] for v in orig.values() if v[0]==3)
assert vb[:len(oldvb)]==oldvb and ib[:len(oldib)]==oldib
assert word*4==len(oldvb) and first*4==len(oldib) and word*4+verts*32==len(vb)
ids=struct.unpack_from('<'+str(count)+'I',ib,first*4); assert min(ids)>=0 and max(ids)<verts
layers=sum(v[0]==4 for v in new.values()); assert layers<=64
pos=[]; leaves=[]; bark_v=[]; rock_vertices=0; village_vertices=[]; prop_vertices=[]; clock_vertices=[]; showcase_vertices=[]; animal_vertices=[]
for i in range(verts):
    at=word*4+i*32
    xyz=struct.unpack_from('<3f',vb,at); assert all(math.isfinite(x) and abs(x)<100 for x in xyz)
    assert struct.unpack_from('<I',vb,at+24)[0]<layers
    kind=vb[at+15]&7
    if kind==4:
        marker=struct.unpack_from('<f',vb,at+28)[0]
        if 100<=marker<=110: village_vertices.append(i)
        elif 111<=marker<=114: prop_vertices.append(i)
        elif marker==115: clock_vertices.append(i)
        elif marker==115.25: showcase_vertices.append(i)
        elif marker==115.375: animal_vertices.append(i)
        else: rock_vertices+=1
    if kind==1:
        ao=struct.unpack_from('<f',vb,at+28)[0]; assert .49<=ao<=1.01
        n=struct.unpack_from('<3b',vb,at+12); assert .96<sum((v/127)**2 for v in n)<1.04
        leaves.append(ao)
    if kind==6:
        u,v=struct.unpack_from('<2H',vb,at+16); bark_v.append(v/65535*64-32)
    pos.append(xyz)
# The zone's pack (VPK2, verse/everglade/glade-zone.coax) has a wider, coarser ground and no sky.
zone=raw[:4]==b'VPK2'
ground_indices=135000 if experimental else (141*92 if zone else 100*100)*6
for i in range(0,ground_indices,3):
    a,b,c=[pos[k] for k in ids[i:i+3]]
    ux,uz=b[0]-a[0],b[2]-a[2]; vx,vz=c[0]-a[0],c[2]-a[2]
    assert uz*vx-ux*vz>0, 'Terrain winding points below hillside'
# The far sky has four directional coordinates; no photographed terrain enters its texture.
if not zone:
    sky_ids=ids[ground_indices:ground_indices+6]; assert len(set(sky_ids))==4
    for i in set(sky_ids): assert vb[word*4+i*32+15]==0
assert leaves and min(leaves)<.9 and bark_v and min(bark_v)<-20
if experimental:
    ground=pos[:151*151]
    assert abs(min(p[0] for p in ground)+90)<.001 and abs(max(p[0] for p in ground)-90)<.001
    assert abs(min(p[2] for p in ground)+90)<.001 and abs(max(p[2] for p in ground)-90)<.001
    assert rock_vertices==0, f"experimental glade retains {rock_vertices} rock vertices"
    if village: assert village_vertices and village_vertices==list(range(village_vertices[0],village_vertices[-1]+1))
    else: assert not village_vertices

# Compare all source layout records and source mesh indices to the published
# scene. The Godot project is read only as archived text; nothing is executed.
if village:
    village_lock=json.loads((repo/'assets/registry/packages/village.json').read_text())
    village_root=Path.home()/'.openagents/assets/originals'/village_lock['revision']/'source'
    with zipfile.ZipFile(village_root/'Engine Projects/Godot.zip') as archive:
        source_scene=archive.read('Godot/addons/quaternius/medievalvillagemegakit/L_SampleScene_1.tscn').decode()
    resources={key:Path(path).stem for path,key in re.findall(r'\[ext_resource[^\]]*path="([^\"]+\.gltf)" id="([^\"]+)"\]',source_scene)}
    groups={'House C':0,'Tower':1,'Houses A':2,'Houses D':3}
    def source_children(name):
        return re.findall(r'\[node name="[^\"]+" parent="'+re.escape(name)+r'" instance=ExtResource\("([^\"]+)"\)\]\n(.*?)(?=\n\[|\Z)',source_scene,re.S)
    assert source_children('House C')==source_children('House C2'), 'sample scene duplicate differs'
    expected=[]
    for m in re.finditer(r'\[node name="[^\"]+" parent="([^\"]+)" instance=ExtResource\("([^\"]+)"\)\]\n(.*?)(?=\n\[|\Z)',source_scene,re.S):
        parent,key,block=m.groups()
        if parent not in groups: continue
        transform=re.search(r'transform = Transform3D\(([^)]*)\)',block)
        values=tuple(map(float,transform.group(1).split(','))) if transform else (1,0,0,0,1,0,0,0,1,0,0,0)
        expected.append((groups[parent],resources[key],values))
    layout=repo/'assets/registry/scenes/everglade-experimental-village.layout'
    actual=[]
    for line in layout.read_text().splitlines():
        group,name,matrix=line.split('|');actual.append((int(group),name,tuple(map(float,matrix.split(',')))))
    assert actual==expected and [sum(r[0]==i for r in actual) for i in range(4)]==[167,77,308,151]
    assert 'L 251 Quaternius Medieval Village MegaKit Source|CC0 1.0 Universal|https://quaternius.com/' in cat.splitlines()
    assert sum(1 for kind,license,_,offset,size,_,name in (struct.unpack_from('<BBHIII32s',raw,32+i*48) for i in range(struct.unpack_from('<I',raw,4)[0])) if kind==4 and license==251)==1
    starts=[(-18,-23),(18,-20),(-50,25),(35,30),(-20,25),(-45,44),(-19,45),(-5,-23),(15,-5)]
    site_groups=[0,1,2,3,3,0,1,3,3]
    tiles={'MI_Brick':0,'MI_FlatTiles':1,'MI_MetalOrnaments':2,'MI_Plaster':3,'MI_RockTrim':4,
           'MI_RoundRocks':5,'MI_RoundTiles':6,'MI_UnevenBrick':7,'MI_Vine':8,'MI_WoodTrim':9,
           'MI_WoodTrim_Wear':9,'MI_WindowGlass':10}
    cache={};vstart=village_vertices[0];vi=vstart;ii=next(k for k,x in enumerate(ids) if x>=vstart)
    placements=[(site,name,t) for group,name,t in actual for site,kind in enumerate(site_groups) if kind==group]
    assert len(placements)==2*167+2*77+308+4*151
    for group,name,t in placements:
        if name not in cache:
            gltf=json.loads((village_root/'glTF'/f'{name}.gltf').read_text())
            blob=(village_root/'glTF'/gltf['buffers'][0]['uri']).read_bytes()
            cache[name]=[(accessor(gltf,blob,p['attributes']['POSITION']),accessor(gltf,blob,p['attributes']['NORMAL']),accessor(gltf,blob,p['attributes']['TEXCOORD_0']),[x[0] for x in accessor(gltf,blob,p['indices'])],tiles[gltf['materials'][p['material']]['name']],gltf['materials'][p['material']].get('pbrMetallicRoughness',{}).get('baseColorFactor',[1,1,1,1])) for mesh in gltf['meshes'] for p in mesh['primitives']]
        sx,sz=starts[group];gx=round((sx+90)/1.2);gz=round((90-sz)/1.2);base=pos[gz*151+gx][1]+.035
        cross=lambda a,b:(a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0])
        # Godot's text writer serializes the basis rows, not its axis columns.
        ca,cb,cc=t[0:9:3],t[1:9:3],t[2:9:3]; na,nb,nc=cross(cb,cc),cross(cc,ca),cross(ca,cb)
        assert sum(ca[d]*na[d] for d in range(3))>0
        for vertices,normals,uvs,triangles,tile,factor in cache[name]:
            for k in {0,len(vertices)//2,len(vertices)-1}:
                v=vertices[k];want=[sx+t[9]+sum(t[d]*v[d] for d in range(3)),base+t[10]+sum(t[3+d]*v[d] for d in range(3)),sz+t[11]+sum(t[6+d]*v[d] for d in range(3))]
                assert all(abs(pos[vi+k][d]-want[d])<.003 for d in range(3)), (group,name,k,pos[vi+k],want)
                off=word*4+(vi+k)*32
                colour=struct.unpack_from('<4B',vb,off+20)
                encode=lambda x: x*12.92 if x<=0.0031308 else 1.055*x**(1/2.4)-0.055
                expected_colour=[round(255*encode(x)) for x in factor[:3]] if tile==8 else [255]*3
                assert all(abs(colour[d]-expected_colour[d])<=1 for d in range(3)), (name,colour,expected_colour)
                normal=[na[d]*normals[k][0]+nb[d]*normals[k][1]+nc[d]*normals[k][2] for d in range(3)]
                mag=math.sqrt(sum(v*v for v in normal)); assert mag>0
                packed=struct.unpack_from('<3b',vb,off+12)
                assert all(abs(packed[d]/127-normal[d]/mag)<.009 for d in range(3)), (group,name,k,packed,normal)
                assert struct.unpack_from('<f',vb,off+28)[0]==100+tile
                uv=struct.unpack_from('<2H',vb,off+16)
                assert all(abs(uv[d]/65535*8-4-uvs[k][d])<.00015 for d in range(2)), (group,name,k,uv,uvs[k])
            assert ids[ii:ii+len(triangles)]==tuple(vi+i for i in triangles), (group,name)
            vi+=len(vertices);ii+=len(triangles)
    assert vi==village_vertices[-1]+1
    if props:
        assert prop_vertices==list(range(vi,prop_vertices[-1]+1))
        props_lock=json.loads((repo/'assets/registry/packages/fantasy-props.json').read_text())
        props_root=Path.home()/'.openagents/assets/originals'/props_lock['revision']/'source/Exports/glTF'
        assert 'L 252 Quaternius Fantasy Props MegaKit Standard|CC0 1.0 Universal|https://quaternius.com/' in cat.splitlines()
        specs=[('Table_Large',1.8,0,.2,0),('Chair_1',1,0,1.15,math.pi),('Chair_1',2.6,0,1.15,math.pi),('Mug',1.8,.82,.2,0),('Book_Stack_1',1.1,.82,.2,.3),('Bed_Twin1',4.6,0,-2.2,math.pi/2),('Workbench',4.7,0,1.2,0),('Barrel_Apples',5,0,5.8,0),('Crate_Wooden',4,0,5.8,.3),('Bucket_Wooden_1',3.3,0,5.6,0),('Stall_Empty',-2,0,7.8,math.pi),('Bench',1.2,0,7.5,0)]
        workshop=[('Workbench_Drawers',3.7,0,-3.8,0),('Stool',3.6,0,-2.8,0),('CandleStick',3.2,.9,-3.8,0),('Key_Metal',3.8,.9,-3.8,0),('Coin_Pile',4.3,.9,-3.8,0),('Bookcase_2',4.3,0,-.6,math.pi/2),('SmallBottles_1',3.3,.9,-3.8,0),('Crate_Wooden',4.4,0,1.8,0),('Workbench',3.8,3,-3.7,0),('Stool',3.5,3,-2.7,0),('Bookcase_2',4.5,3,0,math.pi/2),('BookStand',2,3,1.7,math.pi),('Book_Stack_1',3.3,3.89,-3.7,0),('CandleStick',4.2,3.89,-3.7,0),('Whetstone',2.8,3,1.7,0),('Bench',1.5,0,6,0)]
        prop_tiles={'MI_Trim_Furniture':11,'MI_Trim_Metal':12,'MI_Trim_Metal_Vertex':12,'MI_Trim_Props':13,'MI_Trim_Props_Vertex':13,'MI_Trim_Cloth':14,'MI_Banner':14}
        prop_cache={}
        for site,(sx,sz) in enumerate(starts):
            kind=site_groups[site];shift=-4 if kind==0 else -1 if kind==1 else 0
            for name,px,py,pz,yaw in (workshop if site==8 else specs):
                if name not in prop_cache:
                    d=json.loads((props_root/f'{name}.gltf').read_text());blob=(props_root/d['buffers'][0]['uri']).read_bytes();scale=d['nodes'][0].get('scale',[1,1,1]);parts=[]
                    for mesh in d['meshes']:
                        for p in mesh['primitives']:
                            parts.append((accessor(d,blob,p['attributes']['POSITION']),accessor(d,blob,p['attributes']['TEXCOORD_0']),[x[0] for x in accessor(d,blob,p['indices'])],prop_tiles[d['materials'][p['material']]['name']]))
                    prop_cache[name]=(scale,parts)
                scale,parts=prop_cache[name];cs=math.cos(yaw);sn=math.sin(yaw);base_y=None
                for vertices,uvs,triangles,tile in parts:
                    if base_y is None: base_y=pos[vi][1]-vertices[0][1]*scale[1]
                    for k in {0,len(vertices)//2,len(vertices)-1}:
                        a,b,c=[vertices[k][j]*scale[j] for j in range(3)]
                        want=[sx+shift+px+a*cs+c*sn,base_y+b,sz+pz-a*sn+c*cs]
                        assert all(abs(pos[vi+k][j]-want[j])<.003 for j in range(3)),(site,name,k,pos[vi+k],want)
                        off=word*4+(vi+k)*32;assert struct.unpack_from('<f',vb,off+28)[0]==100+tile
                        uv=struct.unpack_from('<2H',vb,off+16)
                        assert all(abs(uv[j]/65535*8-4-uvs[k][j])<.00015 for j in range(2))
                    assert ids[ii:ii+len(triangles)]==tuple(vi+x for x in triangles)
                    vi+=len(vertices);ii+=len(triangles)
        assert vi==prop_vertices[-1]+1
        assert clock_vertices==list(range(vi,vi+280))
        for i in clock_vertices:
            assert 15.8<=pos[i][0]<=18.2 and abs(pos[i][2]+.55)<.08
        assert all(vi<=v<vi+280 for v in ids[ii:ii+372]);ii+=372;vi+=280
        if showcase:
            assert showcase_vertices==list(range(vi,showcase_vertices[-1]+1))
            rows=(repo/'assets/registry/scenes/everglade-experimental-showcase.layout').read_text().splitlines()
            assert len(rows)==269
            for row in rows[:257] if animal_run else rows:
                package,name,sha=row.split('|');blob=(Path.home()/'.openagents/assets/objects'/sha).read_bytes()
                assert hashlib.sha256(blob).hexdigest()==sha
                modern=('Burger','Cheeseburger','DoubleCheeseburger','Cheese_Singles','ChocolateBar','Chopsticks','Corndog','Cupcake','Donut','Fries','Hotdog','IceCream','Ketchup','MayoBottle','MustardBottle','PeanutButter','Pizza','Popsicle','Sashimi','Soda','SoySauce','Sushi')
                if package=='ultimate-food' and name.startswith(modern): continue
                size=struct.unpack_from('<I',blob,12)[0]; doc=json.loads(blob[20:20+size]); payload=blob[28+size:]
                for node in doc['nodes']:
                    if 'mesh' not in node: continue
                    for primitive in doc['meshes'][node['mesh']]['primitives']:
                        nv=doc['accessors'][primitive['attributes']['POSITION']]['count']
                        triangles=[x[0] for x in accessor(doc,payload,primitive['indices'])]
                        assert ids[ii:ii+len(triangles)]==tuple(vi+x for x in triangles),(package,name)
                        assert showcase_vertices[0]<=vi<=vi+nv-1<=showcase_vertices[-1]
                        vi+=nv;ii+=len(triangles)
            assert vi==showcase_vertices[-1]+1
        else: assert not showcase_vertices
    else: assert not prop_vertices and not clock_vertices
    assert ii==len(ids)

# Verify each instanced template against the pinned glTF, independently of the packer.
names=['Grass_Common_Short','Grass_Wide_Short','Grass_Wispy_Short']
grows=[l.split() for l in cat.splitlines() if l.startswith('C glade/grass')]
assert len(grows)==3
next_index=first+count; heights=[]; instances=[]; template_vertices=0
if animal_run:
    draws=[l.split() for l in cat.splitlines() if l.startswith('C glade/animal')]
    assert len(draws)==10 and animal_vertices
    frame_positions=[]
    for frame,draw in enumerate(draws):
        assert draw[1]==f'glade/animal{frame}'
        af,ac,al,aw,an,aj=map(int,draw[2:]);assert af==next_index and ac>0 and al==253 and aw==word and aj==0
        indices=struct.unpack_from('<'+str(ac)+'I',ib,af*4);start=indices[0]
        assert indices==tuple(range(start,start+ac)) and animal_vertices[0]<=start<start+ac<=animal_vertices[-1]+1
        frame_positions.append([pos[start+i] for i in (0,ac//3,ac//2,ac-1)])
        next_index+=ac
    assert any(frame_positions[0]!=x for x in frame_positions[1:]), 'animal frames are static'
else: assert not animal_vertices
for group,(grow,name) in enumerate(zip(grows,names)):
    assert grow[1]==f'glade/grass{group}'
    gf,gc,gl,gw,gn,gj=map(int,grow[2:]); assert gf==next_index and gl==license and gj==0 and 1<=gn<=65535
    next_index=gf+gc; seedfirst=(gw-word)//8; assert (gw-word)%8==0 and seedfirst+gn<=verts
    doc=json.loads((source/'glTF'/f'{name}.gltf').read_text()); blob=(source/'glTF'/f'{name}.bin').read_bytes()
    pr=doc['meshes'][0]['primitives'][0]
    src_pos=accessor(doc,blob,pr['attributes']['POSITION']); src_uv=accessor(doc,blob,pr['attributes']['TEXCOORD_0'])
    src_indices=[v[0] for v in accessor(doc,blob,pr['indices'])]
    assert len(src_indices)==gc
    packed_ids=struct.unpack_from('<'+str(gc)+'I',ib,gf*4); start=min(packed_ids)
    assert tuple(i+start for i in src_indices)==packed_ids and start+len(src_pos)==seedfirst
    for i,(source_pos,uv) in enumerate(zip(src_pos,src_uv)):
        at=word*4+(start+i)*32
        assert all(abs(a-b)<1e-5 for a,b in zip(pos[start+i],source_pos))
        assert vb[at+15]==2
        packed_uv=struct.unpack_from('<2H',vb,at+16)
        assert all(abs(q/65535-min(1,max(0,s)))<1.6e-5 for q,s in zip(packed_uv,uv))
    maximum=max(v[1] for v in src_pos)
    for i in range(seedfirst,seedfirst+gn):
        at=word*4+i*32; sy,sx=struct.unpack_from('<2H',vb,at+16); sy/=65535; sx/=65535
        assert .001<sy<.5 and .17<=sx<=.29 and vb[at+15]==0
        angle=struct.unpack_from('<f',vb,at+28)[0]; assert math.isfinite(angle) and 0<=angle<6.284
        heights.append(sy*maximum)
    template_vertices+=len(src_pos); instances.append({'source':name,'vertices':len(src_pos),'triangles':gc//3,'instances':gn})
assert next_index*4==len(ib) and seedfirst+gn==verts and sum(v['instances'] for v in instances)>15000
wind_ribbons=sum(1 for i in range(verts) if vb[word*4+i*32+15]==3 and struct.unpack_from('<f',vb,word*4+i*32+28)[0]==2)
assert wind_ribbons==0, 'Removed wind-ribbon geometry must not reappear'
oldcat=next(v[1].decode() for v in orig.values() if v[0]==1)
for line in oldcat.splitlines():
    if line.startswith(('C ','T ')): assert line in cat.splitlines()
for name,(kind,data) in orig.items():
    if kind==4: assert new[name][1]==data
print(json.dumps({'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),'environment_vertices':verts,
 'environment_triangles':count//3,'texture_layers':layers,'wind_ribbons':wind_ribbons,'source_grass_templates':instances,
 'source_grass_instances':sum(v['instances'] for v in instances),'source_grass_topology':'exact source indices and positions',
 'grass_height_range':[min(heights),max(heights)],'bark_repeating_v_range':[min(bark_v),max(bark_v)],
 'rounded_canopy_vertices':len(leaves),'original_character_payloads':'byte-identical','terrain_winding':f'all {ground_indices//3} faces upward', 'rock_vertices':rock_vertices, 'village_vertices':len(village_vertices), 'prop_vertices':len(prop_vertices), 'clock_vertices':len(clock_vertices)},indent=2))
