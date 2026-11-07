# Firm reason: 2 (independent Python oracle for the Coax pack writer's indices, skinning and preservation).
import struct, hashlib, json, sys
from pathlib import Path

def read(path):
 b=Path(path).read_bytes();magic,n,total,budget,table,_,_,_=struct.unpack_from('<4s7I',b);assert magic in (b'VPK1',b'VPK2') and 3<=n<=128 and total==len(b)<=budget
 es=[]
 for i in range(n):
  kind,lic,_,off,size,_,name=struct.unpack_from('<BBHIII32s',b,table+48*i);assert off+size<=len(b);es.append((kind,name.rstrip(b'\0').decode(),b[off:off+size]))
 cat=next(x[2] for x in es if x[0]==1).decode();rows={r.split()[1]:r.split() for r in cat.splitlines() if r.startswith('C ')}
 assert int(next(r.split()[1] for r in cat.splitlines() if r.startswith('bytes ')))==len(b)
 assert int(next(r.split()[1] for r in cat.splitlines() if r.startswith('budget ')))==budget
 return b,es,cat,rows

base,src,out=map(read,sys.argv[1:4]);assert out[0][:4]==b'VPK2';bv=next(e[2] for e in base[1] if e[0]==2);bi=next(e[2] for e in base[1] if e[0]==3);ov=next(e[2] for e in out[1] if e[0]==2);oi=next(e[2] for e in out[1] if e[0]==3);sv=next(e[2] for e in src[1] if e[0]==2);si=next(e[2] for e in src[1] if e[0]==3)
assert ov.startswith(bv) and oi.startswith(bi)
for name,row in base[3].items():assert out[3][name]==row
for e in base[1]:
 if e[0] not in (1,2,3):assert e in out[1]
parts=0;triangles=0;checked_layers=set();out_layers={int(e[1][5:]):e[2] for e in out[1] if e[0]==4};source_layers={int(e[1][5:]):e[2] for e in src[1] if e[0]==4}
for name,row in src[3].items():
 if not name.startswith('fantasy/'):continue
 parts+=1;dest=out[3][name];first,count,lic,word,verts,joints=map(int,dest[2:]);sf,sc,sl,sw,svs,sj=map(int,row[2:]);assert (count,verts,joints)==(sc,svs,65)
 assert word*4+verts*24<=len(ov) and first*4+count*4<=len(oi)
 indices=struct.unpack_from('<'+str(count)+'I',oi,first*4);assert max(indices)<verts
 assert oi[first*4:(first+count)*4]==si[sf*4:(sf+sc)*4]
 for i in range(verts):
  a=ov[word*4+i*24:word*4+(i+1)*24];b=sv[sw*4+i*24:sw*4+(i+1)*24];assert a[:6]==b[:6] and a[7:]==b[7:];assert max(a[16:20])<65 and sum(a[20:24])==255
  if (a[6],b[6]) not in checked_layers:
   for variant in range(3):assert out_layers[a[6]+variant]==source_layers[b[6]+variant];checked_layers.add((a[6]+variant,b[6]+variant))
 triangles+=count//3
assert parts==76
receipt={'base_sha256':hashlib.sha256(base[0]).hexdigest(),'source_sha256':hashlib.sha256(src[0]).hexdigest(),'output_sha256':hashlib.sha256(out[0]).hexdigest(),'output_bytes':len(out[0]),'garments':parts,'garment_triangles':triangles,'original_catalog_vertices_indices_and_images_preserved':True,'local_indices_joints_weights_and_extents_valid':True,'wardrobe_texture_layers_verified':len(checked_layers)}
print(json.dumps(receipt,indent=2))
if len(sys.argv)>4:Path(sys.argv[4]).write_text(json.dumps(receipt,indent=2)+'\n')
