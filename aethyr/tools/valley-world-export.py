# Firm reason: 6 (Unreal Editor's world-partition, mesh and texture SDK has no C ABI).
# Read-only SDK effects in the existing data-only project; never save source packages.
import json
import re
from pathlib import Path
import unreal

root = Path(unreal.Paths.project_saved_dir()) / 'world-source'
root.mkdir(parents=True, exist_ok=True)
unreal.EditorLoadingAndSavingUtils.load_map('/Game/AncientContent/Maps/AncientWorld')
descs = unreal.WorldPartitionBlueprintLibrary.get_actor_descs()
rows = []
for d in descs:
    rows.append({'package': str(d.actor_package), 'path': str(d.actor_path),
                 'label': str(d.label), 'bounds': str(d.bounds),
                 'class': str(d.native_class)})
(root / 'descriptors.json').write_text(json.dumps(rows, indent=2))
def vec(v):
    return [v.x, v.y, v.z]

extend = '-worldextend' in unreal.SystemLibrary.get_command_line()
previous = json.loads((root / 'world.json').read_text()) if extend else {}
meshes = {m['path']: m for m in previous.get('meshes', [])}
instances = previous.get('instances', [])
actors = previous.get('actors', [])
known = {a['package'] for a in actors}
for d in descs:
    if str(d.actor_package) in known:
        continue
    match = re.search(r'MainGrid_L(\d+)_X(-?\d+)_Y(-?\d+)_DL44BCB661', str(d.label))
    bounds = [float(x) for x in re.findall(r'[xyz]: (-?[\d.]+)', str(d.bounds))]
    selected = (match and int(match[1]) == 0 and abs(int(match[2])) <= 1 and abs(int(match[3])) <= 1)
    if extend:
        selected = (match and 1 <= int(match[1]) <= 6 and len(bounds) == 6 and
                    bounds[0] < 12058 and bounds[3] > -13541 and bounds[1] < 13946 and bounds[4] > -11653)
    if not selected:
        continue
    unreal.load_package(str(d.actor_package))
    actor = unreal.find_object(None, str(d.actor_path))
    if not actor:
        raise RuntimeError('Missing selected actor: ' + str(d.actor_path))
    actors.append({'package': str(d.actor_package), 'label': str(d.label)})
    for c in actor.get_components_by_class(unreal.StaticMeshComponent):
        mesh = c.static_mesh
        if not mesh:
            continue
        path = mesh.get_path_name()
        if path not in meshes:
            meshes[path] = {'path': path}
        transforms = ([c.get_instance_transform(i, world_space=True) for i in range(c.get_instance_count())]
                      if isinstance(c, unreal.InstancedStaticMeshComponent) else [c.get_world_transform()])
        for t in transforms:
            q = t.rotation
            instances.append({'mesh': path, 'actor': str(d.label), 'component': c.get_name(),
                              'translation_cm': vec(t.translation), 'quaternion_xyzw': [q.x, q.y, q.z, q.w],
                              'scale': vec(t.scale3d)})
    (root / 'world.json').write_text(json.dumps({'schema': 'valley.world.sdk.v1', 'actors': actors, 'meshes': list(meshes.values()), 'instances': instances}, indent=2))
    unreal.log('VALLEY_WORLD_CELL_DONE %s instances=%d' % (str(d.label), len(instances)))
receipt = {'schema': 'valley.world.sdk.v1', 'engine': unreal.SystemLibrary.get_engine_version(),
           'actors': actors, 'meshes': list(meshes.values()), 'instances': instances}
(root / 'world.json').write_text(json.dumps(receipt, indent=2))
unreal.log('VALLEY_WORLD_INVENTORY_DONE actors=%d meshes=%d instances=%d' % (len(actors), len(meshes), len(instances)))
