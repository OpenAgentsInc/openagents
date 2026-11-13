# Firm reason: 6 (Unreal Editor's serialized scene-component visibility/collision SDK has no C ABI).
# Read authored component properties without reading mesh materials or running the source game.
import json
from pathlib import Path
import unreal

root = Path(unreal.Paths.project_saved_dir()) / 'world-source'
world = json.loads((root / 'world.json').read_text())
packages = {a['package'] for a in world['actors']}
unreal.EditorLoadingAndSavingUtils.load_map('/Game/AncientContent/Maps/AncientWorld')
rows = []
for d in unreal.WorldPartitionBlueprintLibrary.get_actor_descs():
    if str(d.actor_package) not in packages:
        continue
    unreal.load_package(str(d.actor_package))
    actor = unreal.find_object(None, str(d.actor_path))
    for c in actor.get_components_by_class(unreal.StaticMeshComponent):
        if not c.static_mesh:
            continue
        rows.append({'actor': str(d.label), 'component': c.get_name(), 'mesh': c.static_mesh.get_path_name(),
                     'overrides': [m.get_path_name() if m else None for m in c.get_editor_property('override_materials')],
                     'visible': c.get_editor_property('visible'), 'hidden_in_game': c.get_editor_property('hidden_in_game'),
                     'render_in_main_pass': c.get_editor_property('render_in_main_pass'),
                     'cast_shadow': c.get_editor_property('cast_shadow'),
                     'collision_enabled': str(c.get_collision_enabled())})
(root / 'component-flags.json').write_text(json.dumps({'schema': 'valley.world.component-flags.v1', 'components': rows}, indent=2))
lights = []
for d in unreal.WorldPartitionBlueprintLibrary.get_actor_descs():
    if 'DirectionalLight' not in str(d.native_class):
        continue
    unreal.load_package(str(d.actor_package))
    actor = unreal.find_object(None, str(d.actor_path))
    c = actor.get_component_by_class(unreal.DirectionalLightComponent)
    forward = actor.get_actor_forward_vector()
    colour = c.get_editor_property('light_color')
    lights.append({'actor': str(d.label), 'forward_xyz': [forward.x, forward.y, forward.z],
                   'intensity_lux': c.get_editor_property('intensity'),
                   'colour': [colour.r, colour.g, colour.b],
                   'temperature': c.get_editor_property('temperature'),
                   'use_temperature': c.get_editor_property('use_temperature')})
(root / 'lighting.json').write_text(json.dumps({'schema': 'valley.world.sdk-lighting.v1', 'lights': lights}, indent=2))
