# Firm reason: 6 (Unreal Sequencer, Blueprint-template and material SDK access).
# Read only: no world loading, sequence player, construction script or source save.
import json
from pathlib import Path
import unreal

root = Path(unreal.Paths.project_saved_dir()) / 'slay-source'
sequence = unreal.load_asset('/Game/Slay/Sequences/TF/Shots/TF0290/TF0290_01')
span = sequence.make_range(1001, 200)
result = {'sequence': sequence.get_path_name(), 'start': 1001, 'frames': 200, 'fps': 24, 'bindings': []}
for binding in sequence.get_bindings():
    entry = {'name': binding.get_name(), 'tracks': []}
    for track in binding.get_tracks():
        if not isinstance(track, (unreal.MovieScene3DTransformTrack, unreal.MovieSceneFloatTrack)):
            continue
        t = {'name': str(track.get_display_name()), 'sections': []}
        for section in track.get_sections():
            s = {'channels': []}
            for channel in section.get_all_channels():
                if not hasattr(channel, 'evaluate_keys'):
                    continue
                values = channel.evaluate_keys(span, sequence.get_display_rate())
                s['channels'].append({'name': str(channel.channel_name), 'values': list(values)})
            t['sections'].append(s)
        entry['tracks'].append(t)
    result['bindings'].append(entry)
(root / 'tf0290-shot-samples.json').write_text(json.dumps(result, indent=2))
lighting = unreal.load_asset('/Game/Slay/Sequences/TF/Shots/TF0290/TF0290_Lighting')
light_span = lighting.make_range(1, 200)
lights = []
for binding in lighting.get_bindings():
    parent = binding.get_parent()
    entry = {'name': binding.get_name(), 'id': str(binding.get_id()),
             'parent': parent.get_name() if parent.is_valid() else '', 'tracks': []}
    for track in binding.get_tracks():
        t = {'name': str(track.get_display_name()), 'sections': []}
        for section in track.get_sections():
            s = {'channels': []}
            for channel in section.get_all_channels():
                if hasattr(channel, 'evaluate_keys'):
                    s['channels'].append({'name': str(channel.channel_name),
                                          'values': list(channel.evaluate_keys(light_span, lighting.get_display_rate()))})
            t['sections'].append(s)
        entry['tracks'].append(t)
    lights.append(entry)
(root / 'tf0290-light-samples.json').write_text(json.dumps(lights, indent=2))

components = {}
subs = unreal.get_engine_subsystem(unreal.SubobjectDataSubsystem)
for path in ['/Game/Slay/Assets/Chr/Echo/BP_Echo', '/Game/Slay/Assets/Chr/Orb/BP_MoteBase']:
    bp = unreal.load_asset(path)
    rows = []
    for handle in subs.k2_gather_subobject_data_for_blueprint(bp):
        data = unreal.SubobjectDataBlueprintFunctionLibrary.get_data(handle)
        obj = unreal.SubobjectDataBlueprintFunctionLibrary.get_object_for_blueprint(data, bp)
        if not obj:
            continue
        row = {'name': obj.get_name(), 'class': obj.get_class().get_path_name(), 'properties': {}}
        for key in ['relative_location', 'relative_rotation', 'relative_scale3d', 'attach_socket_name',
                    'skeletal_mesh_asset', 'static_mesh', 'override_materials', 'light_color', 'intensity', 'attenuation_radius']:
            try:
                row['properties'][key] = str(obj.get_editor_property(key))
            except Exception:
                pass
        rows.append(row)
        task = unreal.AssetExportTask()
        task.object = obj
        task.filename = str(root / (bp.get_name() + '-' + obj.get_name() + '.t3d'))
        task.exporter = unreal.ObjectExporterT3D()
        task.automated = True
        task.prompt = False
        task.replace_identical = True
        unreal.Exporter.run_asset_export_task(task)
    components[path] = rows
(root / 'shot-components.json').write_text(json.dumps(components, indent=2))
for asset_path in ['/Game/Slay/Assets/FX/Orb/M_Simple_Orb', '/Game/Slay/Assets/Chr/Echo/Meshes/SK_Echo_Sword']:
    asset = unreal.load_asset(asset_path)
    task = unreal.AssetExportTask()
    task.object = asset
    task.filename = str(root / (asset.get_name() + '.t3d'))
    task.exporter = unreal.ObjectExporterT3D()
    task.automated = True
    task.prompt = False
    task.replace_identical = True
    unreal.Exporter.run_asset_export_task(task)
unreal.log('SLAY_SHOT_SAMPLES_RECEIPT')
