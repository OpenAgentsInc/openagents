"""Original coastal kit helpers: meter units, +Z up, fronts toward -Y.

Reference mode: all geometry and colors are authored here. No downloaded
kit, licensed texture, or Fab content is read. Timber uses 2 m modules,
stone has broad irregular facets, and metal details use six-sided profiles.
"""

import hashlib
import json
import math
import os
import random

import bpy
from mathutils import Vector

import kit


def palette():
    materials = {name: kit.mat('Coast_' + name, color, rough) for name, color, rough in [
        ('rock', (0.34, 0.37, 0.36), 0.95),
        ('wood', (0.31, 0.20, 0.12), 0.87),
        ('plank', (0.51, 0.37, 0.22), 0.85),
        ('iron', (0.10, 0.13, 0.14), 0.6),
        ('rope', (0.62, 0.52, 0.32), 0.95),
        ('white', (0.83, 0.81, 0.71), 0.8),
        ('roof', (0.24, 0.31, 0.34), 0.9),
        ('red', (0.63, 0.17, 0.10), 0.8),
        ('green', (0.18, 0.31, 0.16), 0.95),
        ('kelp', (0.21, 0.30, 0.09), 0.9),
        ('shell', (0.83, 0.64, 0.44), 0.75),
    ]}
    image = bpy.data.images.new('Coast_Rock_BaseColor', width=128, height=128)
    rng = random.Random(731)
    pixels = []
    for y in range(128):
        for x in range(128):
            grain = 0.84 + 0.09*math.sin(y*0.22 + math.sin(x*0.08)) + rng.random()*0.12
            pixels.extend([channel*grain for channel in (0.34,0.37,0.36)] + [1.0])
    image.pixels[:] = pixels
    image.pack()
    material = materials['rock']
    node = material.node_tree.nodes.new('ShaderNodeTexImage')
    node.image = image
    material.node_tree.links.new(node.outputs['Color'], material.node_tree.nodes['Principled BSDF'].inputs['Base Color'])
    return materials


def panel(name, points, thickness, material):
    """A closed thin polygon for wings, fins, and plant blades."""
    import bmesh
    bm = bmesh.new()
    top = [bm.verts.new((x,y,z+thickness/2)) for x,y,z in points]
    bottom = [bm.verts.new((x,y,z-thickness/2)) for x,y,z in points]
    bm.faces.new(top); bm.faces.new(list(reversed(bottom)))
    for i in range(len(points)):
        j = (i+1)%len(points)
        bm.faces.new((top[i],bottom[i],bottom[j],top[j]))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    mesh = bpy.data.meshes.new(name); bm.to_mesh(mesh); bm.free()
    obj = bpy.data.objects.new(name,mesh); bpy.context.scene.collection.objects.link(obj)
    mesh.materials.append(material)
    return obj


def beam(name, a, b, radius, material, sides=6):
    a, b = Vector(a), Vector(b)
    delta = b-a
    obj = kit.cyl(name, radius, delta.length, (a+b)/2, material, verts=sides)
    obj.rotation_euler = delta.to_track_quat('Z', 'Y').to_euler()
    return obj


def stone(name, size, location, material, seed=0, subdivisions=2):
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=subdivisions, radius=1)
    obj = bpy.context.object
    obj.name = name
    rng = random.Random(seed)
    for vertex in obj.data.vertices:
        gain = rng.uniform(0.84, 1.12)
        vertex.co.x *= size[0]*0.5*gain
        vertex.co.y *= size[1]*0.5*gain
        vertex.co.z *= size[2]*0.5*gain
    obj.location = location
    obj.data.materials.append(material)
    return obj


def house(p, width=6, length=8, height=3):
    kit.box('Plaster', (width, length, height), (0, 0, height/2), p['white'])
    for x in [-width/2-0.04, width/2+0.04]:
        for y in [-length/2+0.1, 0, length/2-0.1]:
            kit.box('Post', (0.18, 0.18, height), (x, y, height/2), p['wood'])
    pitch = math.radians(35)
    for side in [-1, 1]:
        kit.box('Roof', ((width/2+0.4)/math.cos(pitch), length+0.6, 0.18),
                (side*width/4, 0, height+width/4*math.tan(pitch)), p['roof'],
                rot=(0, side*pitch, 0))
    for y in [-length/2, length/2]:
        mesh = bpy.data.meshes.new('Gable')
        points = [(-width/2,y,height), (width/2,y,height), (0,y,height+width/2*math.tan(pitch))]
        mesh.from_pydata(points, [], [(0,1,2) if y < 0 else (2,1,0)])
        mesh.update()
        obj = bpy.data.objects.new('Gable', mesh)
        bpy.context.scene.collection.objects.link(obj)
        obj.data.materials.append(p['white'])
    kit.box('Door', (1.1, 0.08, 2.1), (0, -length/2-0.05, 1.05), p['wood'])
    for x in [-width/3, width/3]:
        kit.box('Window', (0.75, 0.10, 0.8), (x, -length/2-0.06, 1.8), p['roof'])


def write_metadata(folder, name, triangles):
    """Retain collision surfaces and effective densities for later phases."""
    boxes = []
    if name in ('boathouse', 'keeper_cottage'):
        w, length, top = (6, 9, 6.65) if name == 'boathouse' else (5, 6, 5.5)
        boxes.append({'name': 'body', 'min': [-w/2, 0, -length/2], 'max': [w/2, top, length/2]})
    elif name == 'lighthouse':
        # Strips follow the round tower and the walkable circular plinth.
        for label, radius, top in [('tower', 3.2, 23.35), ('plinth', 4, 1)]:
            for i in range(16):
                x0 = -radius + 2*radius*i/16
                x1 = -radius + 2*radius*(i+1)/16
                reach = math.sqrt(max(0, radius**2-((x0+x1)/2)**2))
                boxes.append({'name': label+str(i), 'walkable': label == 'plinth',
                              'min': [x0, 0, -reach], 'max': [x1, top, reach]})
    elif name == 'pier':
        boxes.append({'name': 'deck', 'walkable': True, 'min': [-1.5, -.08, -2], 'max': [1.5, .08, 2]})
    if boxes:
        record = {'schema': 'openagents.verse.footprint.v1',
                  'frame': {'units': 'meters', 'up': 'Y', 'front': '+Z'},
                  'triangles': triangles, 'boxes': boxes}
        with open(os.path.join(folder, name+'.footprint.json'), 'w') as stream:
            json.dump(record, stream, indent=2); stream.write('\n')


def save(folder, name, build, budget):
    kit.reset()
    build(palette())
    for obj in kit.meshes():
        if not any(mod.type == 'ARMATURE' for mod in obj.modifiers):
            kit.bake(obj)
    triangles = kit.triangles()
    write_metadata(folder, name, triangles)
    if triangles > budget:
        raise ValueError(f'{name}: {triangles} triangles exceeds {budget}')
    info = kit.export(os.path.join(folder, name+'.glb'), animations=bool(bpy.data.actions), extra={
        'mode': 'Reference', 'external_inputs': [], 'triangle_budget': budget,
    })
    path = os.path.join(folder, name+'.glb')
    info['out'] = os.path.basename(folder) + '/' + name + '.glb'
    info['sha256'] = hashlib.sha256(open(path, 'rb').read()).hexdigest()
    density = {'driftwood': 550, 'crate': 220, 'barrel': 180, 'buoy': 250, 'sloop': 450, 'oars': 550}.get(name)
    if density is not None:
        info['effective_density_kg_m3'] = density
    with open(os.path.join(folder, name+'.source.json'), 'w') as stream:
        json.dump(info, stream, indent=2)
        stream.write('\n')


def run(models, family):
    args = kit.args()
    folder = args[0] if args else os.path.join(kit.REPO, 'assets', 'verse', 'generated', 'coast', family)
    os.makedirs(folder, exist_ok=True)
    names = args[1:] or list(models)
    for name in names:
        build, budget = models[name]
        save(folder, name, build, budget)
