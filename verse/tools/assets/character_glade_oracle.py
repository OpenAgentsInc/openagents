# Firm reason: 2 (independent binary-layout/geometry oracle for the Coax glade compiler).
import hashlib,json,math,struct,sys
from pathlib import Path
p=Path(sys.argv[1]);base=Path(sys.argv[2]);raw=p.read_bytes();old=base.read_bytes()
def parts(b):
    magic,n,total,budget,table,*_=struct.unpack_from('<4s7I',b)
    assert magic==b'VPK1' and len(b)==total<=budget<=32*1024*1024
    out={}
    for i in range(n):
        kind,license,_,offset,size,_,name=struct.unpack_from('<BBHIII32s',b,table+i*48)
        assert offset%4==0 and offset+size<=len(b)
        out[name.rstrip(b'\0').decode()]=(kind,b[offset:offset+size])
    return out
new=parts(raw);orig=parts(old)
cat=next(v[1].decode() for v in new.values() if v[0]==1)
assert int(next(l.split()[1] for l in cat.splitlines() if l.startswith('bytes ')))==len(raw)
row=next(l.split() for l in cat.splitlines() if l.startswith('C glade/environment '))
first,count,license,word,verts,joints=map(int,row[2:]);assert joints==0 and license==250
vb=next(v[1] for v in new.values() if v[0]==2);ib=next(v[1] for v in new.values() if v[0]==3)
oldvb=next(v[1] for v in orig.values() if v[0]==2);oldib=next(v[1] for v in orig.values() if v[0]==3)
assert vb[:len(oldvb)]==oldvb and ib[:len(oldib)]==oldib
assert word*4==len(oldvb) and first*4==len(oldib)
assert word*4+verts*32==len(vb) and (first+count)*4==len(ib)
ids=struct.unpack_from('<'+str(count)+'I',ib,first*4);assert min(ids)>=0 and max(ids)<verts
layers=sum(v[0]==4 for v in new.values())
pos=[]
for i in range(verts):
    at=word*4+i*32
    xyz=struct.unpack_from('<3f',vb,at);assert all(math.isfinite(x) and abs(x)<100 for x in xyz)
    layer=struct.unpack_from('<I',vb,at+24)[0];assert layer<layers
    pos.append(xyz)
# The first 101x101 vertices and 100x100 quads form the continuous terrain.
for i in range(0,60000,3):
    a,b,c=[pos[k] for k in ids[i:i+3]]
    ux,uz=b[0]-a[0],b[2]-a[2];vx,vz=c[0]-a[0],c[2]-a[2]
    assert uz*vx-ux*vz>0, 'Terrain winding points below the hillside'
# Verify every original character mesh declaration and every existing image remains unchanged.
oldcat=next(v[1].decode() for v in orig.values() if v[0]==1)
for line in oldcat.splitlines():
    if line.startswith(('C ','T ')):assert line in cat.splitlines()
for name,(kind,data) in orig.items():
    if kind==4:assert new[name][1]==data
print(json.dumps({'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),'environment_vertices':verts,
                  'environment_triangles':count//3,'texture_layers':layers,
                  'original_character_payloads':'byte-identical','terrain_winding':'all 20000 faces upward'},indent=2))
