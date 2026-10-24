#!/usr/bin/env python3
"""Prepare original triangle data for host/artifact conformance, not art QA."""
import json
from pathlib import Path
import struct
import sys

out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
vertices = []
for position, uv in [((-8, 1.55, -8), (0, 0)), ((8, 1.55, -8), (1, 0)), ((0, 1.55, 8), (0.5, 1))]:
    vertices.extend((*position, 0, 1, 0, *uv, 1, 0, 0, 1))
(out / 'vertices.bin').write_bytes(struct.pack('<36f', *vertices))
(out / 'indices.bin').write_bytes(struct.pack('<3I', 0, 2, 1))
(out / 'colors.bin').write_bytes(bytes([255] * 12))
scene = dict(schema='aethyrforge.private-scene.v1', scenario='day', vertices='/private-assets/vertices.bin', indices='/private-assets/indices.bin', vertexColors='/private-assets/colors.bin', vertexStride=48, vertexCount=3, indexCount=3,
    materials=[dict(id='fixture-stone', name='Synthetic stone', baseColorFactor=[0.4, 0.5, 0.6, 1], emissiveFactor=[0, 0, 0], roughness=0.9, metallic=0, normalScale=1, uvScale=[1, 1], alphaMode='OPAQUE', alphaCutoff=0.5, doubleSided=True, packedChannels='metallic-r-roughness-g-occlusion-b')],
    meshes=[dict(id='fixture-ground', name='Synthetic triangle', primitives=[dict(firstIndex=0, indexCount=3, baseVertex=0, material=0, materialSlot=0)], bounds=dict(min=[-8, 1.55, -8], max=[8, 1.55, 8]))],
    instances=[dict(mesh=0, model=[1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1], materialOverrides=[], actor='fixture', component='ground', sourceClass='synthetic')],
    batches=[dict(mesh=0, materialOverrides=[], firstInstance=0, instanceCount=1)], bounds=dict(min=[-8, 1.55, -8], max=[8, 1.55, 8]), spawn=dict(position=[0, 3.17, 5], target=[0, 3.17, 0], source='synthetic'), source=dict(map='conformance', placements=1, actorCensus={}), diagnostics=dict(warnings=[], skippedPlacements=0), catalog='/private-assets/catalog.json')
for mode in ['day', 'night']:
    scene['scenario'] = mode
    (out / ('scene.json' if mode == 'day' else 'scene-night.json')).write_text(json.dumps(scene))
(out / 'catalog.json').write_text(json.dumps(dict(schema='aethyrforge.private-catalog.v1', assets=[], source='original synthetic fixture', limitations=['Not an art review scene'])))
