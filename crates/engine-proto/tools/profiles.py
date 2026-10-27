"""Generate the consumer profile catalog and its compiled C++ byte image."""
import json
from pathlib import Path


def check_compatibility(previous,current):
    now={profile.id:profile for profile in current.profiles}
    for profile in previous.profiles:
        if profile.id not in now or profile.SerializeToString(deterministic=True)!=now[profile.id].SerializeToString(deterministic=True):
            raise ValueError("Published asset profile changed; use a new profile ID: "+profile.id)


def generate(schemas, output):
    from google.protobuf.json_format import ParseDict
    from openagents.engine.v1.assets_pb2 import AssetProfileCatalog
    from portable import safe_path, identifier
    from agent_wire import validate
    catalog=ParseDict(json.loads((schemas/'openagents/engine/v1/profiles.json').read_text()),AssetProfileCatalog())
    validate(catalog)
    if catalog.schema_version!=1 or not catalog.profiles: raise ValueError('Invalid profiles catalog')
    seen=set()
    for profile in catalog.profiles:
        if profile.id in seen or not identifier(profile.id,64) or not identifier(profile.scene_id) or not profile.files: raise ValueError('Invalid profile identity')
        seen.add(profile.id);files={f.id:f for f in profile.files}
        if len(files)!=len(profile.files) or len({f.path for f in profile.files})!=len(files):raise ValueError('Repeated profile file')
        for file in profile.files:
            if not identifier(file.id) or not safe_path(file.path) or not identifier(file.format,64):raise ValueError('Invalid profile file')
            if len(set(file.dependencies))!=len(file.dependencies) or any(d not in files or d==file.id for d in file.dependencies):raise ValueError('Invalid profile dependency')
            for buffer in file.buffers:
                if not identifier(buffer.semantic) or not buffer.byte_stride:raise ValueError('Invalid required buffer')
        def visit(key,active):
            if key in active:raise ValueError('Cyclic profile dependency')
            for dependency in files[key].dependencies:visit(dependency,active|{key})
        for key in files:visit(key,set())
    baseline=Path(__file__).resolve().parents[1]/'compatibility/asset-profiles-v1-20261027.pb'
    if not baseline.is_file():raise ValueError('Published asset profiles baseline is missing')
    check_compatibility(AssetProfileCatalog.FromString(baseline.read_bytes()),catalog)
    data=catalog.SerializeToString(deterministic=True)
    (output/'data/profiles.pb').write_bytes(data)
    (output/'generated/c/openagents/engine/v1/profiles.inc').write_text('// Generated from proto/openagents/engine/v1/profiles.json.\nstatic const uint8_t kEngineAssetProfiles[] = {'+','.join(str(x) for x in data)+'};\n')
