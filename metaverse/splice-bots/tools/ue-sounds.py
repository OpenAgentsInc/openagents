# Firm reason: 6 (the Unreal Editor's asset and data-table API exists only in Python and C++).
# Splice Bots' sounds into the Unreal project: each realms/splice-bots/assets/sounds/NAME.wav imported as
# /Game/Splice/Sounds/NAME and named by the overrides row event.NAME, the row the bridge plays when the
# game reports that sound's event (sound.coax: event 200 + its row, named in the export's manifest).
# The other rows of the table stay as people left them. A loop (a bed) has no stop event on this
# host, so each sound plays once from its start. Run by tools/jam-build.sh in a headless editor:
#   UnrealEditor-Cmd PROJECT -ExecutePythonScript=metaverse/splice-bots/tools/ue-sounds.py   (SPLICE_SOUNDS=dir)
import json
import os
import unreal

SOUNDS = os.environ["SPLICE_SOUNDS"]
DEST = "/Game/Splice/Sounds"
TABLE = "/Game/Splice/Overrides/DT_AethyrOverrides"
assets = unreal.EditorAssetLibrary
lib = unreal.DataTableFunctionLibrary

names = sorted(f[:-4] for f in os.listdir(SOUNDS) if f.endswith(".wav"))
tasks = []
for n in names:
    t = unreal.AssetImportTask()
    t.set_editor_property("filename", os.path.join(SOUNDS, n + ".wav"))
    t.set_editor_property("destination_path", DEST)
    t.set_editor_property("destination_name", n)
    t.set_editor_property("automated", True)
    t.set_editor_property("replace_existing", True)
    t.set_editor_property("save", True)
    tasks.append(t)
unreal.AssetToolsHelpers.get_asset_tools().import_asset_tasks(tasks)

table = assets.load_asset(TABLE)
if table is None:
    raise SystemExit("ue-sounds: no %s (run the importer first)" % TABLE)
rows = {}
for row, value in zip(lib.get_data_table_row_names(table), lib.get_data_table_column_as_string(table, "Asset")):
    rows[str(row)] = str(value)
made = 0
for n in names:
    path = "%s/%s" % (DEST, n)
    if not assets.does_asset_exist(path):
        unreal.log_error("ue-sounds: %s did not import" % n)
        continue
    rows["event." + n] = "%s.%s" % (path, n)
    made += 1
body = json.dumps([{"Name": k, "Asset": v} for k, v in sorted(rows.items())])
if not lib.fill_data_table_from_json_string(table, body):
    raise SystemExit("ue-sounds: the overrides table refused the rows")
assets.save_asset(TABLE, only_if_is_dirty=False)
unreal.log("UE_SOUNDS %d of %d sounds, %d rows" % (made, len(names), len(rows)))
