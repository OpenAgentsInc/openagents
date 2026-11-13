# Firm reason: 6 (the Unreal Editor's asset API exists only in Python and C++).
# A stand-in for a designer, for the exporter's step 3 check (GitHub #13402): puts a different
# model in for the raider and a sound on the pickup event, in /Game/<Game>/Overrides/, the folder
# the importer never writes. Then a re-import must leave all of it as it is.
#   AETHYR_GAME=Splice UnrealEditor-Cmd Project.uproject -ExecutePythonScript=/abs/make_overrides.py -unattended -nosplash -nullrhi
import json
import math
import os
import struct
import tempfile
import wave

import unreal

game = os.environ.get("AETHYR_GAME", "Splice")
folder = "/Game/%s/Overrides" % game
assets = unreal.EditorAssetLibrary
tools = unreal.AssetToolsHelpers.get_asset_tools()

# a model: the engine's cone, copied, twice a person's height
if not assets.does_asset_exist(folder + "/SM_TestRaider"):
    assets.duplicate_asset("/Engine/BasicShapes/Cone", folder + "/SM_TestRaider")

# a sound: a short 880 Hz beep, made here
if not assets.does_asset_exist(folder + "/SW_TestBeep"):
    path = os.path.join(tempfile.mkdtemp(), "SW_TestBeep.wav")
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(22050)
        w.writeframes(b"".join(struct.pack("<h", int(12000 * math.sin(2 * math.pi * 880 * i / 22050.0) * (1.0 - i / 4410.0))) for i in range(4410)))
    task = unreal.AssetImportTask()
    task.filename = path
    task.destination_path = folder
    task.automated = True
    task.save = False
    tools.import_asset_tasks([task])

table = assets.load_asset(folder + "/DT_AethyrOverrides")
rows = [
    {"Name": "thing.raider", "Asset": "%s/SM_TestRaider.SM_TestRaider" % folder},
    {"Name": "event.pickup", "Asset": "%s/SW_TestBeep.SW_TestBeep" % folder},
]
unreal.DataTableFunctionLibrary.fill_data_table_from_json_string(table, json.dumps(rows))
assets.save_directory(folder, only_if_is_dirty=False, recursive=True)
names = [str(n) for n in unreal.DataTableFunctionLibrary.get_data_table_row_names(table)]
out = os.environ.get("AETHYR_REPORT")
if out:
    with open(out, "w") as f:
        json.dump({"rows": names, "assets": sorted(str(d.package_name) for d in unreal.AssetRegistryHelpers.get_asset_registry().get_assets_by_path(folder, recursive=True))}, f)
