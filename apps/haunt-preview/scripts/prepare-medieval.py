#!/usr/bin/env python3
"""Compile local licensed Everglade kit meshes for the Haunted House WebGPU preview.

Inputs and generated files stay outside Git. No vendor geometry is embedded in code.
Requires Pillow. Uses the existing medieval_kit_build.py output, in meters/Y-up.
"""
import hashlib
import json
import math
import os
from pathlib import Path
import struct
from PIL import Image

REPO = Path(__file__).resolve().parents[3]
SOURCE = Path(os.environ.get('HAUNT_MEDIEVAL_SOURCE', str(Path.home()/'.openagents/verse/private/medieval-town/kit-build'))).resolve()
OUT = Path(os.environ.get('HAUNT_MEDIEVAL_OUT', str(Path.home()/'.openagents/verse/private/medieval-town/haunt-webgpu'))).resolve()
if SOURCE.is_relative_to(REPO) or OUT.is_relative_to(REPO):
    raise SystemExit('Licensed kit inputs and compiled files must stay outside the repository.')
OUT.mkdir(parents=True, exist_ok=True)
# Our placement recipes, using the same 8 by 10 meter roof composition as KitHouse.
def house(stories):
    result = []
    for level in range(stories):
        y = level * 4
        for x in [-4, 0]:
            result.append(('door-4' if level == 0 and x == -4 else 'window-4', x, y, 5, 0))
            result.append(('wall-4-timber', -x, y, -5, math.pi))
        for z in [5, 1, -3]:
            piece = 'wall-2' if z == -3 else 'window-4'
            result.append((piece, 4, y, z, math.pi/2))
            result.append((piece, -4, y, -z, -math.pi/2))
    eaves = stories * 4
    result += [('roof-end-mirror', -4, eaves, 0, 0), ('roof-end', 4, eaves, 0, 0),
               ('roof-end', -4, eaves, 0, math.pi), ('roof-end-mirror', 4, eaves, 0, math.pi),
               ('ridge-end-mirror', -5, eaves+3.75, 0, 0), ('ridge-end', 5, eaves+3.75, 0, 0),
               ('ridge-2', 1, eaves+3.75, 0, 0), ('chimney', 2.5, eaves+1, -2, 0)]
    return result
recipes = {'cottage': house(1), 'townhouse': house(2)}
for kind, piece in {'barrel':'barrel','bench':'bench','chair':'chair','market-stall':'stall','cart':'cart','fountain':'fountain'}.items():
    recipes[kind] = [(piece, 0, 0, 0, 0)]
for piece in ['wall-4-timber', 'window-4', 'door-4']:
    recipes['piece/'+piece] = [(piece, 0, 0, 0, 0)]
images = []
image_ids = {}
blob = bytearray()
meshes = {}
source_hashes = {}

def load(path):
    raw = path.read_bytes()
    source_hashes[path.name] = hashlib.sha256(raw).hexdigest()
    return raw

def accessor(doc, buffers, index):
    a = doc['accessors'][index]
    v = doc['bufferViews'][a['bufferView']]
    sizes = {'SCALAR':1, 'VEC2':2, 'VEC3':3, 'VEC4':4}
    code = {5121:'B', 5123:'H', 5125:'I', 5126:'f'}[a['componentType']]
    n = sizes[a['type']]
    fmt = '<'+code*n
    start = v.get('byteOffset', 0)+a.get('byteOffset', 0)
    stride = v.get('byteStride', struct.calcsize(fmt))
    values = [struct.unpack_from(fmt, buffers[v['buffer']], start+i*stride) for i in range(a['count'])]
    if a.get('normalized'):
        divisor = {5121:255, 5123:65535}[a['componentType']]
        values = [tuple(x/divisor for x in row) for row in values]
    return values

for kind, members in recipes.items():
    start = len(blob)//48
    lo, hi = [float('inf')]*3, [float('-inf')]*3
    parts = []
    for piece, x, y, z, yaw in members:
        part_start = len(blob)//48
        doc = json.loads(load(SOURCE/f'{piece}.gltf'))
        buffers = [load(SOURCE/b['uri']) for b in doc['buffers']]
        c, s = math.cos(yaw), math.sin(yaw)
        for primitive in doc['meshes'][0]['primitives']:
            attrs = {key:accessor(doc,buffers,val) for key,val in primitive['attributes'].items()}
            indices = accessor(doc,buffers,primitive['indices'])
            material = doc['materials'][primitive['material']]
            mat = material['pbrMetallicRoughness']
            tex = mat.get('baseColorTexture')
            uri = doc['images'][doc['textures'][tex['index']]['source']]['uri'] if tex else None
            image_key = (uri, material.get("alphaMode", "OPAQUE"))
            if image_key not in image_ids:
                image_ids[image_key] = len(images)
                if uri:
                    load(SOURCE/uri)
                    image = Image.open(SOURCE/uri).convert('RGBA').resize((256,256), Image.Resampling.LANCZOS)
                else: image = Image.new('RGBA', (256,256), (255,255,255,255))
                # Opaque kit textures may use alpha as a material mask, not opacity.
                if image_key[1] == "OPAQUE": image.putalpha(255)
                images.append(image)
            layer = image_ids[image_key]
            factor = mat.get('baseColorFactor',[1,1,1,1])
            for (i,) in indices:
                p = attrs['POSITION'][i]; n = attrs['NORMAL'][i]
                pos = (x+c*p[0]+s*p[2], y+p[1], z-s*p[0]+c*p[2])
                norm = (c*n[0]+s*n[2], n[1], -s*n[0]+c*n[2])
                col = attrs.get('COLOR_0', [(1,1,1,1)]*len(attrs['POSITION']))[i]
                uv = attrs.get('TEXCOORD_0', [(0,0)]*len(attrs['POSITION']))[i]
                rgb = tuple(col[j]*factor[j] for j in range(3))
                blob.extend(struct.pack('<12f',*pos,*norm,*rgb,*uv,layer))
                lo = [min(a,b) for a,b in zip(lo,pos)]; hi = [max(a,b) for a,b in zip(hi,pos)]
        parts.append({'first':part_start,'count':len(blob)//48-part_start,'role':'roof' if any(t in piece for t in ['roof','ridge','chimney']) else 'wall'})
    count = len(blob)//48-start
    # Center props and put the lowest point on the ground. Houses retain real dimensions.
    origin = [0,0,0] if kind.startswith('piece/') else [(lo[0]+hi[0])/2, lo[1], (lo[2]+hi[2])/2]
    for offset in range(start*48, len(blob), 48):
        p = struct.unpack_from('<3f',blob,offset)
        struct.pack_into('<3f',blob,offset,*(p[i]-origin[i] for i in range(3)))
    meshes[kind] = {'first':start,'count':count,'dimensions':[round(hi[i]-lo[i],4) for i in range(3)],'parts':parts}
atlas = Image.new('RGBA',(256*8,256*math.ceil(len(images)/8)))
for i,img in enumerate(images): atlas.paste(img,((i%8)*256,(i//8)*256))
atlas.save(OUT/'atlas.png')
(OUT/'meshes.bin').write_bytes(blob)
files = {name:{'bytes':(OUT/name).stat().st_size,'sha256':hashlib.sha256((OUT/name).read_bytes()).hexdigest()} for name in ['meshes.bin','atlas.png']}
manifest = {'schema':1,'kit':'Modular Medieval Town','source':'Everglade private medieval-town/kit-build','layers':len(images),'meshes':meshes,'files':files,'sources':source_hashes}
(OUT/'catalog.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps({'kit':manifest['kit'],'meshes':len(meshes),'triangles':len(blob)//144,'textureLayers':len(images),'output':str(OUT),'files':files}))
