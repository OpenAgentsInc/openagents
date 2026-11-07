# Firm reason: 2 (independent binary-layout/source-geometry oracle for the Coax glade compiler).
import hashlib, json, math, struct, sys
from pathlib import Path

p = Path(sys.argv[1]); base = Path(sys.argv[2]); raw = p.read_bytes(); old = base.read_bytes()
repo = Path(__file__).resolve().parents[3]
revision = json.loads((repo/'assets/registry/scenes/character-glade.json').read_text())['reference']['revision']
source = Path(sys.argv[3]) if len(sys.argv)>3 else Path.home()/'.openagents/assets/originals'/revision/'source'

def parts(b):
    magic, n, total, budget, table, *_ = struct.unpack_from('<4s7I', b)
    assert magic==b'VPK1' and len(b)==total<=budget<=32*1024*1024
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
first,count,license,word,verts,joints=map(int,row[2:]); assert joints==0 and license==250
vb=next(v[1] for v in new.values() if v[0]==2); ib=next(v[1] for v in new.values() if v[0]==3)
oldvb=next(v[1] for v in orig.values() if v[0]==2); oldib=next(v[1] for v in orig.values() if v[0]==3)
assert vb[:len(oldvb)]==oldvb and ib[:len(oldib)]==oldib
assert word*4==len(oldvb) and first*4==len(oldib) and word*4+verts*32==len(vb)
ids=struct.unpack_from('<'+str(count)+'I',ib,first*4); assert min(ids)>=0 and max(ids)<verts
layers=sum(v[0]==4 for v in new.values()); assert layers<=64
pos=[]; leaves=[]; bark_v=[]
for i in range(verts):
    at=word*4+i*32
    xyz=struct.unpack_from('<3f',vb,at); assert all(math.isfinite(x) and abs(x)<100 for x in xyz)
    assert struct.unpack_from('<I',vb,at+24)[0]<layers
    kind=vb[at+15]&7
    if kind==1:
        ao=struct.unpack_from('<f',vb,at+28)[0]; assert .49<=ao<=1.01
        n=struct.unpack_from('<3b',vb,at+12); assert .96<sum((v/127)**2 for v in n)<1.04
        leaves.append(ao)
    if kind==6:
        u,v=struct.unpack_from('<2H',vb,at+16); bark_v.append(v/65535*64-32)
    pos.append(xyz)
for i in range(0,60000,3):
    a,b,c=[pos[k] for k in ids[i:i+3]]
    ux,uz=b[0]-a[0],b[2]-a[2]; vx,vz=c[0]-a[0],c[2]-a[2]
    assert uz*vx-ux*vz>0, 'Terrain winding points below hillside'
# The far sky has four directional coordinates; no photographed terrain enters its texture.
sky_ids=ids[60000:60006]; assert len(set(sky_ids))==4
for i in set(sky_ids): assert vb[word*4+i*32+15]==0
assert leaves and min(leaves)<.9 and bark_v and min(bark_v)<-20

# Verify each instanced template against the pinned glTF, independently of the packer.
names=['Grass_Common_Short','Grass_Wide_Short','Grass_Wispy_Short']
grows=[l.split() for l in cat.splitlines() if l.startswith('C glade/grass')]
assert len(grows)==3
next_index=first+count; heights=[]; instances=[]; template_vertices=0
for group,(grow,name) in enumerate(zip(grows,names)):
    assert grow[1]==f'glade/grass{group}'
    gf,gc,gl,gw,gn,gj=map(int,grow[2:]); assert gf==next_index and gl==250 and gj==0 and 1<=gn<=65535
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
oldcat=next(v[1].decode() for v in orig.values() if v[0]==1)
for line in oldcat.splitlines():
    if line.startswith(('C ','T ')): assert line in cat.splitlines()
for name,(kind,data) in orig.items():
    if kind==4: assert new[name][1]==data
print(json.dumps({'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),'environment_vertices':verts,
 'environment_triangles':count//3,'texture_layers':layers,'source_grass_templates':instances,
 'source_grass_instances':sum(v['instances'] for v in instances),'source_grass_topology':'exact source indices and positions',
 'grass_height_range':[min(heights),max(heights)],'bark_repeating_v_range':[min(bark_v),max(bark_v)],
 'rounded_canopy_vertices':len(leaves),'original_character_payloads':'byte-identical','terrain_winding':'all 20000 faces upward'},indent=2))
