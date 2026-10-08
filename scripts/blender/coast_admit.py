"""Admit original coastal GLBs without changing geometry or animation.

Usage: python3 scripts/blender/coast_admit.py GENERATED_ROOT OUTPUT_ROOT
Only self-generated, recorded inputs enter this pack. Embedded images are
admitted by digest; files and buffers stay within the output source set.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct

SCHEMA='openagents.verse.source-manifest.v1'
LICENSE='''CC0 1.0 Universal

OpenAgents dedicates these original procedural coast models and textures
to the public domain under CC0 1.0 Universal.
https://creativecommons.org/publicdomain/zero/1.0/
No third-party geometry, images, or licensed kit content is included.
'''


def sha(data): return hashlib.sha256(data).hexdigest()


def unpack(data):
    if len(data)<20 or data[:4]!=b'glTF' or struct.unpack_from('<II',data,4)!=(2,len(data)):
        raise ValueError('Invalid GLB header')
    chunks={}; at=12
    while at<len(data):
        size,kind=struct.unpack_from('<II',data,at);at+=8
        if at+size>len(data) or kind in chunks:raise ValueError('Invalid GLB chunk')
        chunks[kind]=data[at:at+size];at+=size
    return json.loads(chunks[0x4e4f534a]),chunks[0x004e4942]


def admit(source, output):
    source=Path(source); output=Path(output)
    records=list(source.glob('*/*.source.json'))
    if not records:raise ValueError('No generated source records')
    sets={}
    for record_path in sorted(records):
        record=json.loads(record_path.read_text())
        if record.get('mode')!='Reference' or record.get('external_inputs')!=[]:
            raise ValueError(f'{record_path}: expected an original procedural input')
        path=record_path.with_name(record_path.name.removesuffix('.source.json')+'.glb')
        data=path.read_bytes()
        if sha(data)!=record['sha256']:raise ValueError(f'{path}: source digest differs')
        if record['triangles']>record['triangle_budget']:raise ValueError(f'{path}: triangle budget exceeded')
        family='beasts' if path.parent.name=='wildlife' else path.parent.name
        folder=output/family;folder.mkdir(parents=True,exist_ok=True)
        manifest=sets.setdefault(family,{'schema':SCHEMA,'creator':'OpenAgents','license':'CC0-1.0',
            'package':f'Original procedural coast {family}','files':{},'originals':{},'transforms':{}})
        doc,blob=unpack(data)
        # VTP stores base color; the coast renderer assigns lens emission.
        allowed = {'KHR_materials_emissive_strength'}
        if set(doc.get('extensionsUsed', [])) - allowed or doc.get('extensionsRequired'):
            raise ValueError('Unsupported required or unknown glTF extension')
        doc.pop('extensionsUsed', None)
        for material in doc.get('materials', []):
            extensions = material.get('extensions', {})
            extensions.pop('KHR_materials_emissive_strength', None)
            if not extensions: material.pop('extensions', None)
        if len(doc.get('buffers',[]))!=1:raise ValueError('One embedded buffer is required')
        name=path.stem
        doc['buffers'][0]['uri']=name+'.bin'
        files={name+'.bin':blob[:doc['buffers'][0]['byteLength']]}
        for image in doc.get('images',[]):
            if 'uri' in image:raise ValueError('External image inputs are not admitted')
            view=doc['bufferViews'][image.pop('bufferView')]
            start=view.get('byteOffset',0);pixels=blob[start:start+view['byteLength']]
            if image.get('mimeType')!='image/png':raise ValueError('Only embedded PNG images are admitted')
            image.pop('mimeType',None)
            image_name='coast_'+sha(pixels)[:24]+'.png'
            image['uri']=image_name;files[image_name]=pixels
        files[name+'.gltf']=(json.dumps(doc,indent=2,sort_keys=True)+'\n').encode()
        for filename,contents in files.items():
            (folder/filename).write_bytes(contents)
            manifest['files'][filename]=sha(contents)
            manifest['originals'][filename]=sha(data)
            manifest['transforms'][filename]='Split original generated GLB; preserve geometry, base color, skin, and clips; lens emission is assigned by the coast renderer'
    for family,manifest in sets.items():
        folder=output/family
        (folder/'license.txt').write_text(LICENSE)
        digest=sha(LICENSE.encode())
        manifest['files']['license.txt']=digest;manifest['originals']['license.txt']=digest
        (folder/'manifest.json').write_text(json.dumps(manifest,indent=2,sort_keys=True)+'\n')
    print(json.dumps({'models':len(records),'sets':sorted(sets)},indent=2))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source');parser.add_argument('output')
    args=parser.parse_args();admit(args.source,args.output)
