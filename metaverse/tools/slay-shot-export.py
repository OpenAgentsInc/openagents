# Firm reason: 6 (Unreal's editor/Sequencer SDK has no C ABI).
# Read serialized sequence data only; no playback, construction scripts or source saves.
import json
from pathlib import Path
import unreal

root = Path(unreal.Paths.project_saved_dir()) / 'slay-source'
root.mkdir(parents=True, exist_ok=True)
registry = unreal.AssetRegistryHelpers.get_asset_registry()
registry.scan_paths_synchronous(['/Game/Slay/Sequences/TF'], force_rescan=True)
rows = []
for data in registry.get_assets_by_path('/Game/Slay/Sequences/TF', recursive=True):
    if str(data.asset_class_path.asset_name) != 'LevelSequence':
        continue
    sequence = data.get_asset()
    row = {'asset': sequence.get_path_name(), 'start': sequence.get_playback_start(),
           'end': sequence.get_playback_end(), 'rate': str(sequence.get_display_rate()),
           'bindings': []}
    for binding in sequence.get_bindings():
        entry = {'name': binding.get_name(), 'tracks': []}
        for track in binding.get_tracks():
            entry['tracks'].append({'class': track.get_class().get_name(),
                                    'name': str(track.get_display_name()),
                                    'sections': [s.get_class().get_name() for s in track.get_sections()]})
        row['bindings'].append(entry)
    task = unreal.AssetExportTask()
    task.object = sequence
    task.filename = str(root / (sequence.get_name() + '.t3d'))
    task.automated = True
    task.prompt = False
    task.exporter = unreal.ObjectExporterT3D()
    if not unreal.Exporter.run_asset_export_task(task):
        raise RuntimeError('Sequence export refused: ' + sequence.get_path_name())
    rows.append(row)
(root / 'sequences.json').write_text(json.dumps(rows, indent=2))
unreal.log('SLAY_SEQUENCE_RECEIPT ' + str(len(rows)))
