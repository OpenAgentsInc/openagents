# Firm reason: 6 (Blender bpy is required to inspect, export and render editable .blend artwork).
"""Blender SDK bridge. Outputs observations, GLBs and contact sheets; registry policy lives in Coax.
Run only in background with --disable-autoexec. Never execute source-pack scripts.
"""
import bpy, json, sys, math
from pathlib import Path
from mathutils import Vector

def observation(objects, source, selector):
    meshes = [o for o in objects if o.type == 'MESH']
    points = [o.matrix_world @ Vector(v) for o in meshes for v in o.bound_box]
    materials = sorted({m.name for o in meshes for m in o.data.materials if m})
    images = []
    for name in materials:
        m = bpy.data.materials[name]
        if m.use_nodes:
            for n in m.node_tree.nodes:
                if n.type == 'TEX_IMAGE' and n.image:
                    images.append({'name': n.image.name, 'path': n.image.filepath,
                                   'packed': bool(n.image.packed_file), 'size': list(n.image.size)})
    armatures = {m.object.name: m.object for o in meshes for m in o.modifiers if m.type == 'ARMATURE' and m.object}
    return {'blender_version': bpy.app.version_string, 'axes': 'Z-up Blender world; GLB is Y-up', 'source': source, 'selector': selector, 'objects': [o.name for o in meshes],
            'vertices': sum(len(o.data.vertices) for o in meshes),
            'polygons': sum(len(o.data.polygons) for o in meshes),
            'bounds_blender': [[min(p[i] for p in points) for i in range(3)], [max(p[i] for p in points) for i in range(3)]],
            'unit_scale': bpy.context.scene.unit_settings.scale_length,
            'materials': materials, 'images': images,
            'rigs': [{'name': n, 'bones': [b.name for b in a.data.bones]} for n, a in sorted(armatures.items())],
            'libraries': sorted({l.filepath for l in bpy.data.libraries}),
            'collision': 'unreviewed', 'sockets': 'unreviewed'}

def open_source(path):
    bpy.ops.wm.open_mainfile(filepath=str(path), load_ui=False, use_scripts=False)
    for a in bpy.data.armatures:
        a.pose_position = 'REST'
    def enable(layer):
        layer.exclude = False; layer.hide_viewport = False
        for child in layer.children: enable(child)
    enable(bpy.context.view_layer.layer_collection)
    bpy.context.view_layer.update()

def export(objects, dest, origin):
    bpy.ops.object.select_all(action='DESELECT')
    chosen = set(objects)
    for o in objects:
        chosen.update(m.object for m in o.modifiers if m.type == 'ARMATURE' and m.object)
    for o in chosen:
        o.hide_set(False); o.hide_viewport = False; o.hide_select = False; o.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    roots = [o for o in chosen if o.parent not in chosen]
    matrices = {o: o.matrix_world.copy() for o in roots}
    for o in roots:
        matrix = o.matrix_world.copy(); matrix.translation -= origin; o.matrix_world = matrix
    bpy.context.view_layer.update()
    bpy.ops.export_scene.gltf(filepath=str(dest), export_format='GLB', use_selection=True,
                              export_animations=False, export_yup=True)
    for o, matrix in matrices.items(): o.matrix_world = matrix
    bpy.context.view_layer.update()

def sheet(cells, dest):
    # One normalized specimen per cell, with names; Blender performs all image rendering.
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene
    scene.render.engine = 'BLENDER_WORKBENCH'
    scene.display.shading.light = 'STUDIO'; scene.display.shading.color_type = 'MATERIAL'
    scene.display.shading.show_shadows = True; scene.display.shading.show_cavity = True
    scene.display.shading.background_type = 'WORLD'; scene.world = bpy.data.worlds.new('Catalog world'); scene.world.color = (0.08, 0.08, 0.08)
    cols = 8; rows = math.ceil(len(cells) / cols)
    for i, (name, geometry) in enumerate(cells):
        x = (i % cols) * 3.0; y = -(i // cols) * 3.2
        lo = Vector([min(v[k] for verts, faces in geometry for v in verts) for k in range(3)])
        hi = Vector([max(v[k] for verts, faces in geometry for v in verts) for k in range(3)])
        scale = 2.0 / max(max(hi - lo), 0.001); center = (lo + hi) / 2
        for verts, faces in geometry:
            mesh = bpy.data.meshes.new(name)
            mesh.from_pydata([tuple((Vector(v) - center) * scale + Vector((x, y, 0))) for v in verts], [], faces)
            obj = bpy.data.objects.new(name, mesh); scene.collection.objects.link(obj)
        curve = bpy.data.curves.new(name, 'FONT'); curve.body = name; curve.size = min(.15, 2.7 / max(len(name), 1) * 1.5)
        obj = bpy.data.objects.new(name + '_label', curve); scene.collection.objects.link(obj); obj.location = (x-1.25, y-1.5, 0)
    cam_data = bpy.data.cameras.new('Catalog'); cam = bpy.data.objects.new('Catalog', cam_data); scene.collection.objects.link(cam)
    cam.location = ((cols-1)*1.5, -(rows-1)*1.6, 60); cam.rotation_euler = (0, 0, 0)
    cam_data.type = 'ORTHO'; cam_data.ortho_scale = max(cols*3.0, rows*3.2)
    scene.camera = cam; scene.render.resolution_x = 3072; scene.render.resolution_y = max(768, round(3072*rows*3.2/(cols*3)))
    # Orthographic scale is the width for this landscape/portrait aspect.
    cam_data.ortho_scale = cols*3.0 if scene.render.resolution_x >= scene.render.resolution_y else rows*3.2
    scene.render.resolution_percentage = 100; scene.render.image_settings.file_format = 'PNG'
    scene.render.filepath = str(dest); bpy.ops.render.render(write_still=True)

def main():
    job = json.loads(Path(sys.argv[sys.argv.index('--')+1]).read_text())
    if job.get('action') == 'fork':
        open_source(Path(job['source']))
        objects = [o for o in bpy.context.scene.objects if o.type == 'MESH' and (job['selector'] == '*' or o.name == job['selector'])]
        if not objects: raise ValueError('Source selector has no meshes')
        keep = set(objects)
        for o in objects:
            keep.update(m.object for m in o.modifiers if m.type == 'ARMATURE' and m.object)
            parent = o.parent
            while parent: keep.add(parent); parent = parent.parent
        for o in list(bpy.data.objects):
            if o not in keep: bpy.data.objects.remove(o, do_unlink=True)
        bpy.data.orphans_purge(do_recursive=True)
        bpy.ops.file.pack_all()
        bpy.ops.wm.save_as_mainfile(filepath=job['output'], copy=True)
        print('FORKED', job['output'], flush=True)
        return
    root = Path(job['root']); out = Path(job['output']); out.mkdir(parents=True, exist_ok=True)
    records = []; cells = []
    for rel in job['blends']:
        open_source(root / rel)
        meshes = sorted([o for o in bpy.context.scene.objects if o.type == 'MESH'], key=lambda o: o.name)
        if job['mode'] == 'file':
            groups = [(Path(rel).stem, meshes, '*')]
        else:
            groups = [(o.name, [o], o.name) for o in meshes if not job.get('garments') or
                      (o.name.startswith(('Male_', 'Female_')) and 'Regular' not in o.name and len(o.vertex_groups) > 0)]
        for name, objects, selector in groups:
            if not objects: continue
            rec = observation(objects, rel, selector); rec['name'] = name
            filename = name.replace('/', '_').replace('\\', '_') + '.glb'
            origin = Vector((0, 0, 0))
            if job.get('origin_mode') == 'object':
                anchor = objects[0]
                if name.startswith('UCX_'):
                    owner = name[4:].rsplit('_', 1)[0]
                    anchor = bpy.context.scene.objects.get(owner)
                    if anchor is None: raise ValueError('Collision owner missing: '+name)
                origin = anchor.matrix_world.translation.copy()
            rec['export_origin_blender'] = list(origin)
            export(objects, out / filename, origin); rec['export'] = filename
            records.append(rec)
            geometry = []
            deps = bpy.context.evaluated_depsgraph_get()
            for o in objects:
                ev = o.evaluated_get(deps); m = ev.to_mesh()
                geometry.append(([tuple(o.matrix_world @ v.co) for v in m.vertices], [list(p.vertices) for p in m.polygons]))
                ev.to_mesh_clear()
            # Preview rotates Blender Z-up into a three-quarter view on the sheet plane.
            from mathutils import Euler
            rotation = Euler((math.radians(-65), 0, math.radians(-25))).to_matrix()
            geometry = [([tuple(rotation @ Vector(v)) for v in vs], fs) for vs, fs in geometry]
            if not job.get('preview_exclude_prefix') or not name.startswith(job['preview_exclude_prefix']): cells.append((name, geometry))
            print('OBSERVED', name, flush=True)
    with (out / 'observations.jsonl').open('w') as f:
        for rec in records: f.write(json.dumps(rec, sort_keys=True, separators=(',', ':'))+'\n')
    # Page the contact sheets so text remains legible.
    for old in out.glob('contact-*.png'): old.unlink()
    for page, start in enumerate(range(0, len(cells), 64)):
        sheet(cells[start:start+64], out / f'contact-{page+1:02}.png')
    print('COMPLETE', len(records), flush=True)

if __name__ == '__main__': main()
