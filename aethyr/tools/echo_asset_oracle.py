# Firm reason: 2 (independent source-byte, matrix and browser-Wasm oracle; imports no product packer).
import argparse
import hashlib
import json
import math
import mmap
import re
import struct
import subprocess
import tempfile
from pathlib import Path

F = lambda x: struct.unpack('<f', struct.pack('<f', x))[0]
U = lambda b, at=0: struct.unpack_from('<I', b, at)[0]
SHA = lambda b: hashlib.sha256(b).hexdigest()
CLIPS = ['Idle','Walk_Fwd','Jog_Fwd','Jump_Idle_Fall','Jump_Idle_Land','Idle_to_Walk_Fwd','Walk_Fwd_to_Idle','Idle_To_Jog_Fwd','Jog_Fwd_to_Idle','VaultOver','Idle_to_Walk_Fwd_Left_90','Idle_to_Walk_Fwd_Right_90','Idle_to_Walk_Fwd_Left_180','Idle_to_Walk_Fwd_Right_180','Idle_to_Jog_Fwd_Left_90','Idle_to_Jog_Fwd_Right_90','Idle_to_Jog_Fwd_Left_180','Idle_to_Jog_Fwd_Right_180','Idle_Long','Sitting_Idle','StandFromSitting','ReachOut_Start','ReachOut_Loop','ReachOut_End']

class Source:
    def __init__(self, path):
        self.j = json.loads(path.read_text(), parse_int=lambda text: -0.0 if text == "-0" else int(text))
        self.files = [open(path.parent / b['uri'], 'rb') for b in self.j['buffers']]
        self.b = [mmap.mmap(f.fileno(), 0, access=mmap.ACCESS_READ) for f in self.files]

    def accessor(self, index):
        a = self.j['accessors'][index]
        v = self.j['bufferViews'][a['bufferView']]
        assert 'byteStride' not in v
        size = {5121:1,5123:2,5125:4,5126:4}[a['componentType']]
        width = {'SCALAR':1,'VEC2':2,'VEC3':3,'VEC4':4,'MAT4':16}[a['type']]
        at = v.get('byteOffset', 0) + a.get('byteOffset', 0)
        return self.b[v['buffer']][at:at+a['count']*size*width]

def matrix(t, q):
    x,y,z,w = q
    n = math.sqrt(sum(v*v for v in q))
    x,y,z,w = [v/n for v in q]
    return [[1-2*(y*y+z*z),2*(x*y-z*w),2*(x*z+y*w),t[0]],
            [2*(x*y+z*w),1-2*(x*x+z*z),2*(y*z-x*w),t[1]],
            [2*(x*z-y*w),2*(y*z+x*w),1-2*(x*x+y*y),t[2]], [0,0,0,1]]

def mul(a, b):
    return [[sum(a[i][k]*b[k][j] for k in range(4)) for j in range(4)] for i in range(4)]

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--cache', type=Path, default=Path.home()/'.openagents/aethyrforge/valley-lab')
    parser.add_argument('--out', type=Path)
    args = parser.parse_args()
    source = args.cache/'unreal-export/Saved/echo-source'
    echo = args.cache/'echo'
    repository = Path.cwd()
    vb = (echo/'echo.vertices').read_bytes()
    ib = (echo/'echo.indices').read_bytes()
    rig = (echo/'echo.rig').read_bytes()
    slots = json.loads((source/'materials/slots.json').read_text())
    symbols = dict(re.findall(r'(ECHO_\w+_SLOT) :: (\d+);', (repository/'verse/valley/echo-materials.coax').read_text()))
    for symbol, expected in [('ECHO_HEAD_SLOT','M_Head_Echo_Inst'),('ECHO_SKIN_SLOT','MI_Echo_Skin'),('ECHO_LASH_SLOT','M_Lashes_Inst'),('ECHO_EYE_LEFT_SLOT','M_EyeRefractive_Inst'),('ECHO_EYE_RIGHT_SLOT','M_EyeRefractive_Inst')]:
        assert slots['Echo'][int(symbols[symbol])]['asset'].split('.')[-1] == expected, symbol
    sections = json.loads((source/'settings.json').read_text())['sections']
    vertices = indices = 0
    rest = []
    names = []
    parents = []
    bind = []
    for mesh_name, material_ids, joint_base in [('Echo',sections,0),('Echo_Hair',[24,25],134)]:
        g = Source(source/(mesh_name+'.gltf'))
        skin = g.j['skins'][0]
        node_joint = {node: joint_base+i for i,node in enumerate(skin['joints'])}
        node_parent = {child:i for i,n in enumerate(g.j['nodes']) for child in n.get('children',[])}
        inverse = g.accessor(skin['inverseBindMatrices'])
        for i, node_id in enumerate(skin['joints']):
            node = g.j['nodes'][node_id]
            name = node['name']
            parent = node_joint.get(node_parent.get(node_id), -1)
            t, q = node.get('translation',[0,0,0]), node.get('rotation',[0,0,0,1])
            if joint_base and name in names[:134]:
                parent, t, q = names.index(name), [0,0,0], [0,0,0,1]
            expected = struct.pack('<7f',*(t+q))
            at = 416+(joint_base+i)*80
            assert U(rig,at)==parent+1 and rig[at+52:at+80]==expected, (mesh_name,i,name,U(rig,at),parent+1,struct.unpack_from("<7f",rig,at+52),struct.unpack("<7f",expected))
            raw = struct.unpack_from('<16f',inverse,i*64)
            b = [[raw[col*4+row] for col in range(4)] for row in range(4)]
            assert rig[at+4:at+52] == struct.pack('<12f',*(b[r][c] for r in range(3) for c in range(4)))
            names.append(name);parents.append(parent);rest.append(list(struct.unpack('<7f',expected)));bind.append(b)
        for primitive, material in zip(g.j['meshes'][0]['primitives'], material_ids, strict=True):
            a = primitive['attributes']
            n = g.j['accessors'][a['POSITION']]['count']
            base = vertices
            for key, width, offset in [('POSITION',12,0),('NORMAL',12,12),('TEXCOORD_0',8,24),('TANGENT',16,60)]:
                raw = g.accessor(a[key])
                for k in range(n):
                    assert vb[(base+k)*96+offset:(base+k)*96+offset+width] == raw[k*width:(k+1)*width], (mesh_name,key,k)
            for part in range(2):
                raw_j = g.accessor(a[f'JOINTS_{part}']);raw_w = g.accessor(a[f'WEIGHTS_{part}'])
                for k in range(n):
                    at=(base+k)*96
                    assert vb[at+36+part*8:at+44+part*8] == struct.pack('<4H',*(x+joint_base for x in raw_j[k*4:k*4+4]))
                    assert vb[at+52+part*4:at+56+part*4] == raw_w[k*4:k*4+4]
                    assert U(vb,at+32)==material
            raw = g.accessor(primitive['indices']);kind=g.j['accessors'][primitive['indices']]['componentType']
            values=[x[0] for x in struct.iter_unpack({5121:'<B',5123:'<H',5125:'<I'}[kind],raw)]
            assert ib[indices*4:(indices+len(values))*4] == struct.pack('<'+str(len(values))+'I',*(v+base for v in values))
            vertices+=n;indices+=len(values)
    assert vertices==313371 and indices==1470318 and len(rig)==10142528
    groom=(echo/'echo.groom').read_bytes()
    assert U(groom)==0x32475345 and U(groom,4)==492907
    segment=0;curves_total=points_total=0
    for asset,name in enumerate(['Hair_S_UpdoBuns','Eyebrows_L_Echo']):
        data=(source/(name+'.strands')).read_bytes();cursor=4;arrays=[]
        for width in [12,4,4,2,4,8]:
            n=U(data,cursor);cursor+=4;arrays.append((n,data[cursor:cursor+n*width]));cursor+=n*width
        assert cursor==len(data)
        position,radius,coord,count,offset,uv=arrays
        rs=[x[0] for x in struct.iter_unpack('<f',radius[1])];maximum=max(rs)
        points=list(struct.iter_unpack('<3f',position[1]));starts=[x[0] for x in struct.iter_unpack('<I',offset[1])]
        curves_total+=count[0];points_total+=position[0]
        for curve in range(count[0]):
            for point in range(starts[curve],starts[curve+1]-1):
                at=32+segment*48
                for endpoint in range(2):
                    i=point+endpoint;x,y,z=points[i]
                    converted=struct.pack('<3f',F(x*F(.01)),F(z*F(.01)),F(y*F(.01)))
                    assert groom[at+endpoint*16:at+endpoint*16+12]==converted
                    r=F(F(rs[i]/maximum)*F(.00005 if asset==0 else .00004))
                    assert groom[at+endpoint*16+12:at+endpoint*16+16]==struct.pack('<f',r)
                    assert groom[at+32+endpoint*4:at+36+endpoint*4]==coord[1][i*4:i*4+4]
                assert U(groom,at+44)==26+asset
                segment+=1
    assert segment==492907 and curves_total==45091 and points_total==537998
    frames_total=0
    for clip, name in enumerate(CLIPS):
        g=Source(source/(name+'.gltf'));anim=g.j['animations'][0]
        first, frames, duration, speed=struct.unpack_from('<IIff',rig,32+clip*16)
        for channel in anim['channels']:
            node=g.j['nodes'][channel['target']['node']];joint=names.index(node['name']);path=channel['target']['path']
            sampler=anim['samplers'][channel['sampler']];raw=g.accessor(sampler['output'])
            assert g.j['accessors'][sampler['output']]['count']==frames
            if path=='scale':
                assert all(abs(v[0]-1)<1e-6 for v in struct.iter_unpack('<f',raw));continue
            width,offset={'translation':(12,0),'rotation':(16,12)}[path]
            for k in range(frames):
                at=first+(k*134+joint)*28+offset
                assert rig[at:at+width]==raw[k*width:(k+1)*width], (name,joint,path,k)
        frames_total+=frames
    # Independently compose matrices, then compare the actual browser Wasm's initial source pose.
    first=U(rig,32)
    poses=[list(struct.unpack_from('<7f',rig,first+j*28)) if j<134 else rest[j] for j in range(334)]
    global_m=[];palette=[]
    for j,pose in enumerate(poses):
        local=matrix(pose[:3],pose[3:]);p=parents[j]
        world=local if p<0 else mul(global_m[p],local)
        global_m.append(world)
        result=mul(world,bind[j]);palette.extend(result[r][c] for r in range(3) for c in range(4))
    with tempfile.TemporaryDirectory(prefix='echo-oracle-') as temporary:
        output=Path(temporary)/'palette.bin'
        js="""import fs from 'node:fs';const [wasm,rig,out]=process.argv.slice(1);const memory=new WebAssembly.Memory({initial:1024,maximum:1024});const {instance}=await WebAssembly.instantiate(fs.readFileSync(wasm),{env:{memory},wasi_snapshot_preview1:{fd_fdstat_get:()=>0,fd_seek:()=>0,fd_write:()=>0}});const x=instance.exports;x._initialize();const p=Number(x.__heap_base.value);x.coaxial_state_init(p);const b=fs.readFileSync(rig);new Uint8Array(memory.buffer,x.coaxial_asset(p,0,b.length),b.length).set(b);x.coaxial_asset_ready(p,0,b.length);fs.writeFileSync(out,new Uint8Array(memory.buffer,x.coaxial_store(p,0),x.coaxial_store_bytes(p,0)));"""
        subprocess.run(['node','--input-type=module','-e',js,str(repository/'apps/openagents-web/build/valleycanyon.wasm'),str(echo/'echo.rig'),str(output)],check=True)
        observed=struct.unpack('<4008f',output.read_bytes())
        error=max(abs(a-b) for a,b in zip(palette,observed,strict=True))
        assert error<0.00001,error
    # The combined pack preserves both original source outcrops and all original PNG bytes.
    with open(echo/'valley-echo.pack','rb') as file:
        pack=mmap.mmap(file.fileno(),0,access=mmap.ACCESS_READ)
        entries=[(U(pack,32+i*48),U(pack,36+i*48),U(pack,40+i*48)) for i in range(U(pack,4))]
        assert pack[entries[0][1]:entries[0][1]+entries[0][2]]==rig
        vertex_offset=entries[1][1];index_offset=entries[2][1]
        for name,offset in [('canyon.vertices.bin',vertex_offset),('canyon.indices.bin',index_offset)]:
            raw=(args.cache/'canyon-pair'/name).read_bytes();assert pack[offset:offset+len(raw)]==raw
        assert pack[vertex_offset+24209616+2048:vertex_offset+54295280]==vb
        assert pack[vertex_offset+54295280:vertex_offset+54295280+len(groom)]==groom
        expected_indices=[x[0]+504367 for x in struct.iter_unpack('<I',ib)]
        assert pack[index_offset+11999940:index_offset+17881212]==struct.pack('<1470318I',*expected_indices)
        assert pack[index_offset+17881212:index_offset+29710980]==struct.pack('<2957442I',*range(1000000,3957442))
        maps=re.search(r'EB_MAPS :: \[30\]string\.\[(.*?)\];',(repository/'verse/tools/valley-echo-bundle.coax').read_text(),re.S).group(1)
        maps=['Albedo.png','Roughness.png']+re.findall(r'"([^"]+)"',maps)
        for i,name in enumerate(maps):
            directory=args.cache/'unreal-export/Saved/section-source' if i<2 else source
            raw=(directory/name).read_bytes();_,at,n=entries[3+i];assert pack[at:at+n]==raw,name
        pack_sha=SHA(pack)
    receipt={'vertices_bit_exact':vertices,'indices_bit_exact':indices,'influences_per_vertex':8,'clips_bit_exact':24,'source_frames_bit_exact':frames_total,'joints':334,'material_slots_match_sdk':symbols,'wasm_palette_max_absolute_error':error,'groom_curves':curves_total,'groom_points_checked':points_total,'groom_segments_checked':segment,'original_png_layers_bit_exact':len(maps),'pack_sha256':pack_sha,'rig_sha256':SHA(rig),'vertices_sha256':SHA(vb),'indices_sha256':SHA(ib),'groom_sha256':SHA(groom)}
    text=json.dumps(receipt,indent=2)+'\n'
    if args.out:args.out.write_text(text)
    print(text,end='')

if __name__=='__main__':
    main()
