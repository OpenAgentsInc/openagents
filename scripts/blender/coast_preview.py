"""Render original coast model previews with Eevee, one model at a time.

Run under a GPU lease: Blender -b --python SCRIPT -- SOURCE_ROOT OUTPUT_ROOT.
This renders raster previews and never uses Cycles or a lighting bake.
"""
from pathlib import Path
import sys
sys.dont_write_bytecode = True
import bpy
from mathutils import Vector

source,out=map(Path,sys.argv[sys.argv.index('--')+1:][:2])
for path in sorted(source.glob('*/*.glb')):
    if path.parent.name=='lod' and '--lod' not in sys.argv:continue
    if '--match' in sys.argv and sys.argv[sys.argv.index('--match')+1] not in path.stem:continue
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=str(path))
    bpy.context.scene.frame_set(1)
    bpy.context.view_layer.update()
    objects=[obj for obj in bpy.context.scene.objects if obj.type=='MESH']
    graph=bpy.context.evaluated_depsgraph_get()
    points=[]
    for obj in objects:
        evaluated=obj.evaluated_get(graph); mesh=evaluated.to_mesh()
        points.extend(evaluated.matrix_world@vertex.co for vertex in mesh.vertices)
        evaluated.to_mesh_clear()
    low=Vector([min(p[i] for p in points) for i in range(3)])
    high=Vector([max(p[i] for p in points) for i in range(3)])
    center=(low+high)/2;radius=(high-low).length
    camera=bpy.data.objects.new('PreviewCamera',bpy.data.cameras.new('PreviewCamera'))
    bpy.context.scene.collection.objects.link(camera)
    camera.location=center+Vector((radius*.85,-radius, radius*.7))
    camera.rotation_euler=(center-camera.location).to_track_quat('-Z','Y').to_euler()
    camera.data.type='ORTHO';camera.data.ortho_scale=radius*1.12
    scene=bpy.context.scene;scene.camera=camera
    sun=bpy.data.objects.new('PreviewSun',bpy.data.lights.new('PreviewSun','SUN'))
    sun.data.energy=2;sun.rotation_euler=(.7,-.3,-.4)
    scene.collection.objects.link(sun)
    scene.world=bpy.data.worlds.new('PreviewWorld');scene.world.color=(.22,.26,.30)
    scene.render.engine='BLENDER_EEVEE'
    scene.render.resolution_x=384;scene.render.resolution_y=288
    scene.render.resolution_percentage=100
    target=out/path.parent.name/(path.stem+'.png');target.parent.mkdir(parents=True,exist_ok=True)
    scene.render.filepath=str(target)
    bpy.ops.render.render(write_still=True)
