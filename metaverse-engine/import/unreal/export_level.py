# Firm reason: 6 (Unreal Editor's world, mesh, material, texture, sound and glTF export SDK has no C ABI).
# The editor route of the Unreal importer: writes one import bundle (BUNDLE.md, version 1) for the
# maps of a UE 5.9 project, from inside a headless editor. Read-only: it never saves a package.
# Raw values only: every number is what Unreal reports, in Unreal's frame; the Coax side converts.
#   UnrealEditor-Cmd PROJECT.uproject -ExecutePythonScript=export_level.py -unattended -nosplash
#     -RenderOffscreen -nosound -NoSourceControl -abslog=LOG
# Driven by metaverse-engine/tools/unreal-level-export.coax, which sets the environment:
#   AIMPORT_OUT       the new, empty bundle folder (NAME.aimport)
#   AIMPORT_MAPS      comma-separated map packages, or empty for every map under /Game except the
#                     folders in AIMPORT_EXCLUDE
#   AIMPORT_EXCLUDE   comma-separated /Game folders whose maps are not taken (default Test, Dev,
#                     ThirdPerson templates)
#   AIMPORT_PROJECT, AIMPORT_REVISION, AIMPORT_PLAN_SHA256   copied into manifest.source (the plan's SHA-256 is required)
#   AIMPORT_SOURCE_RIGS=1 when the caller imports rigs and clips from source files separately.
#   Level instances still carry the skeletal meshes they reference.
# Material baking needs a renderer: under -nullrhi the glTF exporter bakes nothing and says
# nothing, so the script refuses -nullrhi rather than write unbaked materials as baked.
# Simple-collision convex hulls and skeleton parents are not readable from Python; the
# AethyrBridge plugin's UAethyrEditorChecks (ReadConvex, ReadBones) reads them, and the script
# refuses to run without it.
# Ends with "AIMPORT_DONE" and the counts in the log, or "AIMPORT_FAILED reason" and no manifest.
import hashlib
import json
import os
import re
import struct
import traceback
import zlib
import unreal

OUT = os.environ.get('AIMPORT_OUT', '')
TEMPLATE_FOLDERS = ('/Game/Characters/Mannequins/', '/Game/LevelPrototyping/', '/Game/ThirdPerson/', '/Game/StarterContent/')
DEFAULT_EXCLUDE = '/Game/World/Test,/Game/World/Dev,/Game/Dev,/Game/Test,/Game/ThirdPerson'
MANNEQUIN_CORE = {'root', 'pelvis', 'spine_01', 'spine_02', 'spine_03', 'neck_01', 'head', 'clavicle_l', 'upperarm_l',
                  'lowerarm_l', 'hand_l', 'clavicle_r', 'upperarm_r', 'lowerarm_r', 'hand_r', 'thigh_l', 'calf_l', 'foot_l',
                  'ball_l', 'thigh_r', 'calf_r', 'foot_r', 'ball_r'}
# Actor classes that are content, not behaviour. Any other class (a Blueprint or a game's C++
# class) is carried through its components and gets one skipped row with reason "logic".
CONTENT_ACTORS = {
    '/Script/Engine.StaticMeshActor', '/Script/Engine.SkeletalMeshActor', '/Script/Engine.Actor',
    '/Script/Engine.DirectionalLight', '/Script/Engine.PointLight', '/Script/Engine.SpotLight', '/Script/Engine.RectLight',
    '/Script/Engine.SkyLight', '/Script/Engine.SkyAtmosphere', '/Script/Engine.ExponentialHeightFog',
    '/Script/Engine.VolumetricCloud', '/Script/Engine.PostProcessVolume', '/Script/Engine.PlayerStart',
    '/Script/Engine.TargetPoint', '/Script/Engine.BlockingVolume', '/Script/Engine.TriggerVolume',
    '/Script/Engine.LevelInstance', '/Script/Engine.WorldDataLayers', '/Script/Engine.WorldPartitionMiniMap',
    '/Script/Engine.InstancedFoliageActor', '/Script/Engine.Note',
}
# Actors that are not content and not game behaviour: navigation, cameras, sequences, captures.
UNSUPPORTED_ACTORS = {
    '/Script/NavigationSystem.RecastNavMesh': 'navigation mesh', '/Script/NavigationSystem.NavMeshBoundsVolume': 'navigation bounds',
    '/Script/AIModule.NavLinkProxy': 'navigation link', '/Script/CinematicCamera.CineCameraActor': 'camera',
    '/Script/Engine.CameraActor': 'camera', '/Script/Engine.SceneCapture2D': 'scene capture',
}
SEQUENCER_ACTORS = {'/Script/LevelSequence.LevelSequenceActor'}
EDITOR_ONLY_COMPONENTS = {'BillboardComponent', 'ArrowComponent', 'DrawFrustumComponent', 'CameraProxyMeshComponent',
                          'NavLinkRenderingComponent', 'NavMeshRenderingComponent', 'NavTestRenderingComponent',
                          'SplineComponent', 'LineBatchComponent', 'MaterialBillboardComponent'}
ASSET_SKIPS = {'MetaSoundSource': ('audio_graph', 'MetaSound'), 'SoundCue': ('audio_graph', 'sound cue'),
               'SoundAttenuation': ('audio_graph', 'attenuation'), 'NiagaraSystem': ('niagara', 'Niagara system'),
               'LevelSequence': ('sequencer', 'level sequence'), 'AnimBlueprint': ('logic', 'animation blueprint'),
               'Blueprint': ('logic', 'Blueprint'), 'WidgetBlueprint': ('widget', 'widget Blueprint'),
               'BlendSpace': ('unsupported', 'blend space'), 'AimOffsetBlendSpace': ('unsupported', 'aim offset'),
               'AnimMontage': ('unsupported', 'animation montage'), 'ControlRigBlueprint': ('logic', 'Control Rig'),
               'IKRigDefinition': ('unsupported', 'IK rig'), 'IKRetargeter': ('unsupported', 'IK retargeter'),
               'PhysicsAsset': ('unsupported', 'physics asset')}


class Refusal(Exception):
    pass


def log(text):
    unreal.log('AIMPORT ' + text)


def num(v):
    v = round(float(v), 6)
    return 0.0 if v == 0 else v


def vec3(v):
    return [num(v.x), num(v.y), num(v.z)]


def quat(q):
    return [num(q.x), num(q.y), num(q.z), num(q.w)]


def origin_of(path):
    if not path.startswith('/Game/'):
        return 'engine'
    return 'engine' if path.startswith(TEMPLATE_FOLDERS) else 'project'


def carried(path):
    """A bundle id is under /Game/ or /Engine/ (BUNDLE.md 3); a plugin's content (/InterchangeAssets/...) is not."""
    return bool(path) and path.startswith(('/Game/', '/Engine/'))


def obj_id(o):
    return o.get_path_name() if o else None


def soft_path(x):
    if hasattr(x, 'get_path_name'):
        return x.get_path_name()
    if hasattr(x, 'export_text'):
        return x.export_text().strip('"')
    return str(x)


def short(package):
    return package.rsplit('/', 1)[-1]


# --- Quaternion transforms (FTransform composition: world = child then parent) -------------

def qmul(a, b):
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return [aw * bx + ax * bw + ay * bz - az * by, aw * by - ax * bz + ay * bw + az * bx,
            aw * bz + ax * by - ay * bx + az * bw, aw * bw - ax * bx - ay * by - az * bz]


def qrot(q, v):
    x, y, z, w = q
    tx, ty, tz = 2 * (y * v[2] - z * v[1]), 2 * (z * v[0] - x * v[2]), 2 * (x * v[1] - y * v[0])
    return [v[0] + w * tx + (y * tz - z * ty), v[1] + w * ty + (z * tx - x * tz), v[2] + w * tz + (x * ty - y * tx)]


def compose(child, parent):
    """child and parent are (translation, rotation, scale); returns child placed under parent."""
    ct, cr, cs = child
    pt, pr, ps = parent
    scaled = [ct[0] * ps[0], ct[1] * ps[1], ct[2] * ps[2]]
    moved = qrot(pr, scaled)
    return ([moved[0] + pt[0], moved[1] + pt[1], moved[2] + pt[2]], qmul(pr, cr), [cs[0] * ps[0], cs[1] * ps[1], cs[2] * ps[2]])


def trs(t):
    return (vec3(t.translation), quat(t.rotation), vec3(t.scale3d))


def put_trs(row, t):
    row['translation_cm'] = [num(x) for x in t[0]]
    row['rotation'] = [num(x) for x in t[1]]
    if 'scale' in row:
        row['scale'] = [num(x) for x in t[2]]


# --- File readers (headers of what Unreal wrote) -------------------------------------------

def glb_json(path):
    with open(path, 'rb') as f:
        data = f.read()
    if data[:4] != b'glTF':
        raise Refusal('not a glTF binary: ' + path)
    n = struct.unpack_from('<I', data, 12)[0]
    return json.loads(data[20:20 + n]), data


def glb_counts(j):
    tris = 0
    verts = 0
    prims = 0
    for m in j.get('meshes', []):
        for p in m['primitives']:
            prims += 1
            if 'indices' in p:
                tris += j['accessors'][p['indices']]['count'] // 3
            verts += j['accessors'][p['attributes']['POSITION']]['count']
    return tris, verts, prims


def png_header(path):
    with open(path, 'rb') as f:
        head = f.read(33)
    if head[:8] != b'\x89PNG\r\n\x1a\n':
        raise Refusal('not a PNG: ' + path)
    w, h, depth = struct.unpack('>IIB', head[16:25])
    return w, h, depth


def wav_header(path):
    with open(path, 'rb') as f:
        data = f.read(4096)
    if data[:4] != b'RIFF' or data[8:12] != b'WAVE':
        raise Refusal('not a WAV: ' + path)
    i = 12
    while i + 8 <= len(data):
        tag, size = data[i:i + 4], struct.unpack_from('<I', data, i + 4)[0]
        if tag == b'fmt ':
            channels, rate = struct.unpack_from('<HI', data, i + 10)
            return channels, rate
        i += 8 + size
    raise Refusal('WAV without fmt: ' + path)


def png_1x1(path, rgba):
    raw = b'\x00' + bytes(max(0, min(255, int(round(c * 255)))) for c in rgba)

    def chunk(tag, body):
        return struct.pack('>I', len(body)) + tag + body + struct.pack('>I', zlib.crc32(tag + body) & 0xffffffff)
    with open(path, 'wb') as f:
        f.write(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 1, 1, 8, 6, 0, 0, 0)) +
                chunk(b'IDAT', zlib.compress(raw, 9)) + chunk(b'IEND', b''))


# --- The run ----------------------------------------------------------------------------------

class Export:
    def __init__(self):
        self.ar = unreal.AssetRegistryHelpers.get_asset_registry()
        self.skipped = []
        self.maps = {}            # package -> collected rows in the map's own frame
        self.static_ids = set()
        self.skeletal_ids = set()
        self.material_ids = set()
        self.texture_ids = set()
        self.anim_ids = set()
        self.sound_ids = set()
        self.skeleton_ids = set()
        self.checks = unreal.AethyrEditorChecks if hasattr(unreal, 'AethyrEditorChecks') else None

    def skip(self, map_id, actor, cls, component, reason, detail='', asset=''):
        row = {'map': map_id, 'actor': actor, 'class': cls, 'component': component, 'reason': reason}
        if asset:
            detail = (detail + ': ' if detail else '') + asset
        if detail:
            row['detail'] = detail
        self.skipped.append(row)

    def assets_of(self, cls_name, script='/Script/Engine'):
        flt = unreal.ARFilter(class_paths=[unreal.TopLevelAssetPath(script, cls_name)], package_paths=['/Game'], recursive_paths=True)
        return sorted(self.ar.get_assets(flt), key=lambda a: str(a.package_name))

    # ---- maps ----
    def map_list(self):
        named = [m.strip() for m in os.environ.get('AIMPORT_MAPS', '').split(',') if m.strip()]
        if named:
            return sorted(set(named))
        exclude = [e.strip().rstrip('/') + '/' for e in os.environ.get('AIMPORT_EXCLUDE', DEFAULT_EXCLUDE).split(',') if e.strip()]
        worlds = self.assets_of('World')
        return sorted(str(a.package_name) for a in worlds if not any(str(a.package_name).startswith(e) for e in exclude))

    def load_all_actors(self, map_id):
        descs = unreal.WorldPartitionBlueprintLibrary.get_actor_descs()
        if not descs:
            return {}
        own = [d for d in descs if not unreal.GuidLibrary.is_valid_guid(d.parent_instance_guid)]
        missing = [d for d in own if not unreal.find_object(None, str(d.actor_path))]
        if missing:
            unreal.WorldPartitionBlueprintLibrary.load_actors([d.guid for d in missing])
            still = [str(d.actor_path) for d in missing if not unreal.find_object(None, str(d.actor_path))]
            if still:
                raise Refusal('%s: %d World Partition actors did not load, first %s' % (map_id, len(still), still[0]))
        layers = {}
        for d in own:
            assets = sorted(soft_path(x) for x in (d.data_layer_assets or []) if soft_path(x))
            if assets:
                layers[str(d.actor_path)] = assets
        log('%s world partition actors=%d (in level instances %d)' % (map_id, len(own), len(descs) - len(own)))
        return layers

    def collect_map(self, map_id):
        if not unreal.EditorLoadingAndSavingUtils.load_map(map_id):
            raise Refusal('map did not load: ' + map_id)
        for cmd in ('Editor.AsyncStaticMeshCompilationFinishAll', 'Editor.AsyncTextureCompilationFinishAll'):
            unreal.SystemLibrary.execute_console_command(None, cmd)
        world = unreal.EditorLevelLibrary.get_editor_world()
        persistent = world.get_path_name() + ':PersistentLevel'
        layers = self.load_all_actors(map_id)
        actors = unreal.get_editor_subsystem(unreal.EditorActorSubsystem).get_all_level_actors()
        rows = {'placements': [], 'lights': [], 'spots': [], 'volumes': [], 'landscapes': [], 'atmos': [],
                'instances': [], 'streamed': set(), 'skipped': []}
        for actor in sorted(actors, key=lambda a: (a.get_outer().get_path_name(), a.get_name())):
            level = actor.get_outer().get_path_name()
            prefix = ''
            if level != persistent:
                package = level.split('.')[0]
                rows['streamed'].add(package)
                prefix = short(package) + '.'
            self.collect_actor(map_id, actor, prefix + actor.get_name(), layers.get(actor.get_path_name(), []), rows)
        return rows

    def collect_actor(self, map_id, actor, name, layers, rows):
        cls = actor.get_class().get_path_name()
        skip = rows['skipped']
        # data layers (time of day, for example) ride in the tags as datalayer:PATH
        tags = [str(t) for t in actor.tags] + ['datalayer:' + l for l in layers]
        extra = {'data_layers': layers} if layers else {}
        try:
            label = actor.get_actor_label()
        except Exception:
            label = ''
        if cls in SEQUENCER_ACTORS:
            skip.append((name, cls, '', 'sequencer', ''))
            return
        if cls in UNSUPPORTED_ACTORS:
            skip.append((name, cls, '', 'unsupported', UNSUPPORTED_ACTORS[cls]))
            return
        if cls == '/Script/Engine.AmbientSound':
            skip.append((name, cls, '', 'audio_graph', 'ambient sound'))
            return
        if isinstance(actor, unreal.LevelInstance):
            world = actor.get_editor_property('world_asset')
            if not world:
                skip.append((name, cls, '', 'unsupported', 'level instance without a world'))
                return
            rows['instances'].append({'name': name, 'world': world.get_path_name().split('.')[0], 'transform': trs(actor.get_actor_transform())})
            return
        try:
            hidden_actor = bool(actor.get_editor_property('hidden'))
        except Exception:
            hidden_actor = False
        actor_t = trs(actor.get_actor_transform())
        aethyr = [t for t in tags if t.startswith('aethyr.')]
        for t in aethyr:
            rows['spots'].append({'name': t[len('aethyr.'):], 'kind': 'tag', 'translation_cm': actor_t[0], 'rotation': actor_t[1], 'tags': tags})
        if isinstance(actor, unreal.PlayerStart):
            rows['spots'].append({'name': name, 'kind': 'player_start', 'translation_cm': actor_t[0], 'rotation': actor_t[1], 'tags': tags})
        elif isinstance(actor, unreal.TargetPoint):
            rows['spots'].append({'name': name, 'kind': 'target_point', 'translation_cm': actor_t[0], 'rotation': actor_t[1], 'tags': tags})
        if isinstance(actor, (unreal.BlockingVolume, unreal.TriggerVolume)):
            self.collect_volume(actor, name, cls, actor_t, rows)
            return
        if isinstance(actor, unreal.PostProcessVolume):
            if actor.get_editor_property('unbound'):
                rows['atmos'].append(('post', name, layers, self.post_numbers(actor)))
            else:
                skip.append((name, cls, '', 'unsupported', 'bound post process volume'))
            return
        if isinstance(actor, unreal.Volume):
            skip.append((name, cls, '', 'unsupported', 'volume'))
            return
        if isinstance(actor, unreal.LandscapeProxy):
            skip.append((name, cls, '', 'unsupported', 'landscape (no landscape reader yet)'))
            return
        if cls not in CONTENT_ACTORS:
            skip.append((name, cls, '', 'logic', ''))
        lights = [c for c in actor.get_components_by_class(unreal.LightComponentBase)]
        for comp in sorted(actor.get_components_by_class(unreal.ActorComponent), key=lambda c: c.get_name()):
            self.collect_component(map_id, actor, name, cls, comp, tags, label, hidden_actor, extra, len(lights), rows)

    def collect_component(self, map_id, actor, name, cls, comp, tags, label, hidden_actor, extra, light_count, rows):
        skip = rows['skipped']
        cname = comp.get_name()
        kind = comp.get_class().get_name()
        if kind in EDITOR_ONLY_COMPONENTS:
            return
        try:
            if comp.get_editor_property('is_editor_only'):
                return
        except Exception:
            pass
        if isinstance(comp, unreal.LightComponentBase):
            self.collect_light(name if light_count == 1 else name + '/' + cname, comp, extra, rows)
            return
        if isinstance(comp, unreal.SkyAtmosphereComponent):
            rows['atmos'].append(('sky_atmosphere', name, extra.get('data_layers', []), self.props(comp, SKY_ATMOSPHERE_PROPS)))
            return
        if isinstance(comp, unreal.ExponentialHeightFogComponent):
            rows['atmos'].append(('fog', name, extra.get('data_layers', []), self.props(comp, FOG_PROPS)))
            return
        if isinstance(comp, unreal.VolumetricCloudComponent):
            rows['atmos'].append(('cloud', name, extra.get('data_layers', []), self.props(comp, CLOUD_PROPS)))
            return
        if hasattr(unreal, 'NiagaraComponent') and isinstance(comp, unreal.NiagaraComponent):
            skip.append((name, cls, cname, 'niagara', ''))
            return
        if isinstance(comp, unreal.AudioComponent):
            skip.append((name, cls, cname, 'audio_graph', 'audio component'))
            return
        if isinstance(comp, unreal.DecalComponent):
            skip.append((name, cls, cname, 'decal', ''))
            return
        if hasattr(unreal, 'WidgetComponent') and isinstance(comp, unreal.WidgetComponent):
            skip.append((name, cls, cname, 'widget', ''))
            return
        if not isinstance(comp, unreal.PrimitiveComponent):
            return
        if isinstance(comp, unreal.SplineMeshComponent):
            skip.append((name, cls, cname, 'unsupported', 'spline mesh (deformed along its spline)'))
            return
        if isinstance(comp, unreal.StaticMeshComponent):
            mesh = comp.static_mesh
            if not mesh:
                skip.append((name, cls, cname, 'unsupported', 'static mesh component without a mesh'))
                return
            asset, kind_out = obj_id(mesh), 'static'
            self.static_ids.add(asset)
        elif isinstance(comp, unreal.SkinnedMeshComponent):
            mesh = comp.get_skinned_asset() if hasattr(comp, 'get_skinned_asset') else comp.get_editor_property('skinned_asset')
            if not mesh or not isinstance(mesh, unreal.SkeletalMesh):
                skip.append((name, cls, cname, 'unsupported', 'skinned component without a skeletal mesh'))
                return
            asset, kind_out = obj_id(mesh), 'skeletal'
            self.skeletal_ids.add(asset)
        elif isinstance(comp, unreal.BrushComponent):
            return
        elif isinstance(comp, unreal.ShapeComponent):
            skip.append((name, cls, cname, 'unsupported', 'shape component: ' + kind))
            return
        else:
            skip.append((name, cls, cname, 'unsupported', 'primitive component: ' + kind))
            return
        hidden = hidden_actor or bool(comp.get_editor_property('hidden_in_game')) or not comp.is_visible()
        collision = self.collision_mode(comp)
        if hidden and collision == 'none':
            skip.append((name, cls, cname, 'hidden', ''))
            return
        materials = [obj_id(m) for m in comp.get_materials()]
        for m in materials:
            if m:
                self.material_ids.add(m)
        all_tags = tags + [str(t) for t in comp.component_tags]
        extra = {}
        base = {'actor': name, 'actor_class': cls, 'component': cname, 'kind': kind_out, 'asset': asset,
                'materials': materials, 'collision': collision, 'visible': not hidden,
                'cast_shadow': bool(comp.get_editor_property('cast_shadow')), 'tags': all_tags}
        if isinstance(comp, unreal.InstancedStaticMeshComponent):
            for i in range(comp.get_instance_count()):
                row = dict(base, name='%s/%s#%d' % (name, cname, i), instance=i)
                row['_t'] = trs(comp.get_instance_transform(i, True))
                rows['placements'].append(row)
        else:
            row = dict(base, name='%s/%s' % (name, cname), instance=-1)
            row['_t'] = trs(comp.get_world_transform())
            rows['placements'].append(row)

    def collision_mode(self, comp):
        enabled = comp.get_collision_enabled()
        if enabled in (unreal.CollisionEnabled.NO_COLLISION, unreal.CollisionEnabled.PHYSICS_ONLY):
            return 'none'
        response = comp.get_collision_response_to_channel(unreal.CollisionChannel.ECC_PAWN)
        if response == unreal.CollisionResponseType.ECR_BLOCK:
            return 'block'
        if response == unreal.CollisionResponseType.ECR_OVERLAP:
            return 'overlap'
        return 'none'

    def collect_light(self, name, comp, extra, rows):
        c = comp.get_editor_property('light_color')
        row = {'name': name, 'color': [num(c.r / 255.0), num(c.g / 255.0), num(c.b / 255.0)],
               'intensity': num(comp.get_editor_property('intensity')), 'radius_cm': 0, 'inner_deg': 0, 'outer_deg': 0,
               'cast_shadows': bool(comp.get_editor_property('cast_shadows')),
               'movable': str(comp.get_editor_property('mobility')).split('.')[-1].split(':')[0].strip('<> ').lower()}
        if isinstance(comp, unreal.SkyLightComponent):
            row.update(kind='sky', units='unitless')
        elif isinstance(comp, unreal.DirectionalLightComponent):
            row.update(kind='directional', units='lux')
        else:
            units = str(comp.get_editor_property('intensity_units')).split('.')[-1].split(':')[0].strip('<> ').lower()
            row['units'] = units
            row['radius_cm'] = num(comp.get_editor_property('attenuation_radius'))
            if isinstance(comp, unreal.SpotLightComponent):
                row.update(kind='spot', inner_deg=num(comp.get_editor_property('inner_cone_angle')), outer_deg=num(comp.get_editor_property('outer_cone_angle')))
            elif isinstance(comp, unreal.RectLightComponent):
                row['kind'] = 'rect'
            elif isinstance(comp, unreal.PointLightComponent):
                row['kind'] = 'point'
            else:
                rows['skipped'].append((name, comp.get_class().get_path_name(), comp.get_name(), 'unsupported', 'light class'))
                return
        row.update(extra)
        row['_t'] = trs(comp.get_world_transform())
        row['_order'] = ['name', 'kind', 'translation_cm', 'rotation', 'color', 'intensity', 'units', 'radius_cm', 'inner_deg', 'outer_deg', 'cast_shadows', 'movable']
        rows['lights'].append(row)

    def collect_volume(self, actor, name, cls, actor_t, rows):
        builder = actor.get_editor_property('brush_builder')
        if isinstance(builder, unreal.CubeBuilder):
            box = {'center_cm': [0.0, 0.0, 0.0], 'rotation': [0.0, 0.0, 0.0, 1.0],
                   'extent_cm': [num(builder.get_editor_property(k) / 2.0) for k in ('x', 'y', 'z')]}
        else:
            origin, extent = actor.get_actor_bounds(False)
            # world bounds back into the actor's frame, axis-aligned: exact only for an unrotated brush
            rows['skipped'].append((name, cls, '', 'unsupported', 'non-box brush carried as its bounds'))
            box = {'center_cm': [num(origin.x - actor_t[0][0]), num(origin.y - actor_t[0][1]), num(origin.z - actor_t[0][2])],
                   'rotation': [0.0, 0.0, 0.0, 1.0], 'extent_cm': vec3(extent)}
        rows['volumes'].append({'name': name, 'class': cls, '_t': actor_t, 'boxes': [box], 'scale': [1, 1, 1]})

    def props(self, comp, names):
        out = {}
        for n in names:
            try:
                out[n] = plain(comp.get_editor_property(n))
            except Exception:
                pass
        return out

    def post_numbers(self, actor):
        settings = actor.get_editor_property('settings').to_dict()
        out = {}
        for k, v in sorted(settings.items()):
            if k.startswith('override_') and v:
                key = k[len('override_'):]
                if key in settings:
                    out[key] = plain(settings[key])
        return out

    # ---- flatten level instances into their host maps ----
    def flatten(self, map_id, stack=()):
        rows = self.maps[map_id]
        out = {k: list(rows[k]) for k in ('placements', 'lights', 'spots', 'volumes', 'atmos', 'skipped')}
        streamed = set(rows['streamed'])
        for inst in rows['instances']:
            world = inst['world']
            if world in stack:
                raise Refusal('level instance cycle: ' + ' > '.join(stack + (world,)))
            inner = self.flatten(world, stack + (world,))
            streamed.add(world)
            streamed |= set(inner['streamed'])
            pivot = self.pivot_offset(world)
            parent = compose(((pivot[0], pivot[1], pivot[2]), [0, 0, 0, 1], [1, 1, 1]), inst['transform'])
            for key in ('placements', 'lights', 'spots', 'volumes'):
                for r in inner[key]:
                    r = dict(r)
                    r['_t'] = compose(r['_t'] if '_t' in r else (r['translation_cm'], r['rotation'], [1, 1, 1]), parent)
                    r['name'] = inst['name'] + '.' + r['name']
                    if 'actor' in r:
                        r['actor'] = inst['name'] + '.' + r['actor']
                    if key == 'spots':
                        r['translation_cm'], r['rotation'] = r['_t'][0], r['_t'][1]
                    out[key].append(r)
            for a in inner['atmos']:
                out['atmos'].append((a[0], inst['name'] + '.' + a[1], a[2], a[3]))
            for s in inner['skipped']:
                out['skipped'].append((inst['name'] + '.' + s[0],) + tuple(s[1:]))
        out['streamed'] = streamed
        return out

    def pivot_offset(self, world):
        data = self.ar.get_asset_by_object_path(world + '.' + short(world))
        text = str(data.get_tag_value('LevelInstancePivotOffset')) if data else ''
        found = [float(x) for x in re.findall(r'[XYZ]=(-?[\d.eE+-]+)', text)]
        return found if len(found) == 3 else [0.0, 0.0, 0.0]

    # ---- assets ----
    def add_material_closure(self):
        pending = sorted(self.material_ids)
        while pending:
            path = pending.pop()
            m = unreal.load_asset(path)
            if not m:
                raise Refusal('material did not load: ' + path)
            if isinstance(m, unreal.MaterialInstance) and m.parent:
                p = obj_id(m.parent)
                if not carried(p):
                    # a plugin's material: its instance is carried flattened, as its own root (material_row)
                    self.skip('', '', m.parent.get_class().get_path_name(), '', 'unsupported',
                              'plugin material, flattened into the instance %s' % path, p)
                    continue
                if p not in self.material_ids:
                    self.material_ids.add(p)
                    pending.append(p)

    def export_mesh(self, mesh, path, file_rel):
        nanite = bool(mesh.get_editor_property('nanite_settings').enabled)
        o = unreal.GLTFExportOptions()
        o.export_source_model = nanite
        o.bake_material_inputs = unreal.GLTFMaterialBakeMode.DISABLED
        o.texture_image_format = unreal.GLTFTextureImageFormat.NONE
        o.export_vertex_colors = True
        o.default_level_of_detail = 0
        result = unreal.GLTFExporter.export_to_gltf(mesh, os.path.join(OUT, file_rel), o, set())
        if result.errors:
            raise Refusal('glTF export of %s: %s' % (path, '; '.join(map(str, result.errors))))
        j, _ = glb_json(os.path.join(OUT, file_rel))
        tris, verts, prims = glb_counts(j)
        if nanite:
            desc = mesh.get_static_mesh_description(0)
            want_t, want_v = desc.get_triangle_count(), desc.get_vertex_count()
        else:
            want_t, want_v = mesh.get_num_triangles(0), mesh.get_num_vertices(0)
        if tris != want_t:
            raise Refusal('%s: glTF holds %d triangles, Unreal reports %d (%s)' % (path, tris, want_t, 'source' if nanite else 'LOD 0'))
        box = mesh.get_bounding_box()
        slots = [obj_id(s.material_interface) for s in mesh.static_materials]
        for m in slots:
            if m:
                self.material_ids.add(m)
        row = {'id': path, 'file': file_rel, 'triangles': want_t, 'vertices': want_v,
               'bounds_cm': vec3(box.min) + vec3(box.max), 'nanite': nanite, 'materials': slots,
               'collision': self.mesh_collision(mesh, path), 'origin': origin_of(path)}
        return row

    def mesh_collision(self, mesh, path):
        out = {'complex_as_simple': False, 'boxes': [], 'spheres': [], 'capsules': [], 'convex': []}
        body = mesh.get_editor_property('body_setup')
        if not body:
            return out
        out['complex_as_simple'] = body.get_editor_property('collision_trace_flag') == unreal.CollisionTraceFlag.CTF_USE_COMPLEX_AS_SIMPLE
        agg = body.get_editor_property('agg_geom')
        for b in agg.get_editor_property('box_elems'):
            out['boxes'].append({'center_cm': vec3(b.get_editor_property('center')), 'rotation': quat(b.get_editor_property('rotation').quaternion()),
                                 'extent_cm': [num(b.get_editor_property(k) / 2.0) for k in ('x', 'y', 'z')]})
        for s in agg.get_editor_property('sphere_elems'):
            out['spheres'].append({'center_cm': vec3(s.get_editor_property('center')), 'radius_cm': num(s.get_editor_property('radius'))})
        for c in agg.get_editor_property('sphyl_elems'):
            out['capsules'].append({'center_cm': vec3(c.get_editor_property('center')), 'rotation': quat(c.get_editor_property('rotation').quaternion()),
                                    'radius_cm': num(c.get_editor_property('radius')), 'length_cm': num(c.get_editor_property('length'))})
        if len(agg.get_editor_property('tapered_capsule_elems')):
            self.skip('', '', 'StaticMesh', '', 'unsupported', 'tapered capsule collision', path)
        hulls = len(agg.get_editor_property('convex_elems'))
        if hulls:
            got = self.checks.read_convex(mesh)
            ok, points, counts = got if len(got) == 3 else (True,) + tuple(got)
            if not ok or len(counts) != hulls:
                raise Refusal('%s: convex hulls unreadable' % path)
            at = 0
            for n in counts:
                out['convex'].append({'vertices_cm': [vec3(p) for p in points[at:at + n]]})
                at += n
        return out

    def export_skeletal(self, mesh, path, file_rel):
        o = unreal.GLTFExportOptions()
        o.bake_material_inputs = unreal.GLTFMaterialBakeMode.DISABLED
        o.texture_image_format = unreal.GLTFTextureImageFormat.NONE
        o.export_vertex_skin_weights = True
        o.export_vertex_colors = True
        o.default_level_of_detail = 0
        o.export_morph_targets = True
        result = unreal.GLTFExporter.export_to_gltf(mesh, os.path.join(OUT, file_rel), o, set())
        if result.errors:
            raise Refusal('glTF export of %s: %s' % (path, '; '.join(map(str, result.errors))))
        j, _ = glb_json(os.path.join(OUT, file_rel))
        tris, verts, _ = glb_counts(j)
        if not j.get('skins'):
            raise Refusal('%s: glTF has no skin' % path)
        skeleton = obj_id(mesh.get_editor_property('skeleton'))
        self.skeleton_ids.add(skeleton)
        slots = [obj_id(m.material_interface) for m in mesh.get_editor_property('materials')]
        for m in slots:
            if m:
                self.material_ids.add(m)
        b = mesh.get_bounds()
        lo = [b.origin.x - b.box_extent.x, b.origin.y - b.box_extent.y, b.origin.z - b.box_extent.z]
        hi = [b.origin.x + b.box_extent.x, b.origin.y + b.box_extent.y, b.origin.z + b.box_extent.z]
        return {'id': path, 'file': file_rel, 'skeleton': skeleton, 'triangles': tris, 'vertices': verts,
                'bounds_cm': [num(x) for x in lo + hi], 'materials': slots, 'origin': origin_of(path)}

    def export_anim(self, anim, path, file_rel):
        o = unreal.GLTFExportOptions()
        o.export_preview_mesh = False
        o.bake_material_inputs = unreal.GLTFMaterialBakeMode.DISABLED
        o.texture_image_format = unreal.GLTFTextureImageFormat.NONE
        result = unreal.GLTFExporter.export_to_gltf(anim, os.path.join(OUT, file_rel), o, set())
        if result.errors:
            raise Refusal('glTF export of %s: %s' % (path, '; '.join(map(str, result.errors))))
        j, _ = glb_json(os.path.join(OUT, file_rel))
        if len(j.get('animations', [])) != 1:
            raise Refusal('%s: glTF holds %d clips, not 1' % (path, len(j.get('animations', []))))
        frames = unreal.AnimationLibrary.get_num_frames(anim)
        seconds = unreal.AnimationLibrary.get_sequence_length(anim)
        skeleton = obj_id(anim.get_editor_property('skeleton'))
        self.skeleton_ids.add(skeleton)
        return {'id': path, 'file': file_rel, 'skeleton': skeleton, 'frames': frames,
                'rate': round(frames / seconds, 3) if seconds > 0 else 0, 'seconds': num(seconds), 'origin': origin_of(path)}

    def skeleton_row(self, path):
        sk = unreal.load_asset(path)
        got = self.checks.read_bones(sk)
        ok, names, parents, rest = got if len(got) == 4 else (True,) + tuple(got)
        if not ok or not names:
            raise Refusal('%s: skeleton unreadable' % path)
        bones = [{'name': str(n), 'parent': int(p), 'rest_translation_cm': vec3(t.translation), 'rest_rotation': quat(t.rotation),
                  'rest_scale': vec3(t.scale3d)} for n, p, t in zip(names, parents, rest)]
        origin = origin_of(path)
        if MANNEQUIN_CORE <= {str(n) for n in names}:
            origin = 'engine'
        return {'id': path, 'origin': origin, 'bones': bones}

    def material_row(self, path):
        m = unreal.load_asset(path)
        mel = unreal.MaterialEditingLibrary
        instance = isinstance(m, unreal.MaterialInstance)
        root = m
        while isinstance(root, unreal.MaterialInstance) and root.parent:
            root = root.parent
        if not isinstance(root, unreal.Material):
            raise Refusal('%s: no root material' % path)
        blend = root.get_editor_property('blend_mode')
        two_sided = bool(root.get_editor_property('two_sided'))
        shading = root.get_editor_property('shading_model')
        if instance:
            # the instance chain's overrides, nearest first
            chain = []
            at = m
            while isinstance(at, unreal.MaterialInstance):
                chain.append(at)
                at = at.parent
            for mi in reversed(chain):
                o = mi.get_editor_property('base_property_overrides')
                if o.get_editor_property('override_blend_mode'):
                    blend = o.get_editor_property('blend_mode')
                if o.get_editor_property('override_two_sided'):
                    two_sided = bool(o.get_editor_property('two_sided'))
                if o.get_editor_property('override_shading_model'):
                    shading = o.get_editor_property('shading_model')
        blend_name = {unreal.BlendMode.BLEND_OPAQUE: 'opaque', unreal.BlendMode.BLEND_MASKED: 'masked',
                      unreal.BlendMode.BLEND_TRANSLUCENT: 'translucent', unreal.BlendMode.BLEND_ADDITIVE: 'additive'}.get(blend)
        if not blend_name:
            blend_name = 'translucent'
            self.skip('', '', m.get_class().get_path_name(), '', 'unsupported', 'blend mode %s carried as translucent' % str(blend), path)
        shading_name = {unreal.MaterialShadingModel.MSM_DEFAULT_LIT: 'default_lit', unreal.MaterialShadingModel.MSM_UNLIT: 'unlit',
                        unreal.MaterialShadingModel.MSM_SUBSURFACE: 'subsurface', unreal.MaterialShadingModel.MSM_SUBSURFACE_PROFILE: 'subsurface',
                        unreal.MaterialShadingModel.MSM_PREINTEGRATED_SKIN: 'subsurface'}.get(shading, 'other')
        # a root material is its own parent, so every material names one; an instance of a plugin's
        # material is carried flattened, as its own root, with its parameters' values and its own bake
        flat = instance and not carried(obj_id(m.parent))
        row = {'id': path, 'parent': obj_id(m.parent) if instance and not flat else path, 'root': path if flat else obj_id(root), 'blend': blend_name,
               'two_sided': two_sided, 'shading': shading_name, 'textures': [], 'scalars': [], 'vectors': [], 'baked': {}}
        for n in sorted(set(str(x) for x in mel.get_texture_parameter_names(m))):
            t = mel.get_material_instance_texture_parameter_value(m, n) if instance else mel.get_material_default_texture_parameter_value(root, n)
            if t and not isinstance(t, unreal.Texture2D):
                self.skip('', '', t.get_class().get_path_name(), '', 'unsupported', 'texture parameter %s of %s is not a 2D texture' % (n, path), obj_id(t))
                t = None
            if t and not carried(obj_id(t)):
                self.skip('', '', t.get_class().get_path_name(), '', 'unsupported', 'texture parameter %s of %s is plugin content' % (n, path), obj_id(t))
                t = None
            if t:
                # a parameter with no texture set has nothing to carry
                self.texture_ids.add(obj_id(t))
                row['textures'].append({'name': n, 'texture': obj_id(t)})
        for n in sorted(set(str(x) for x in mel.get_scalar_parameter_names(m))):
            v = mel.get_material_instance_scalar_parameter_value(m, n) if instance else mel.get_material_default_scalar_parameter_value(root, n)
            row['scalars'].append({'name': n, 'value': num(v)})
        for n in sorted(set(str(x) for x in mel.get_vector_parameter_names(m))):
            c = mel.get_material_instance_vector_parameter_value(m, n) if instance else mel.get_material_default_vector_parameter_value(root, n)
            row['vectors'].append({'name': n, 'value': [num(c.r), num(c.g), num(c.b), num(c.a)]})
        row['origin'] = origin_of(path)
        return row

    def bake(self, material, path, number, baked_textures):
        """Bakes a non-instance material with the glTF exporter. Returns the baked map, or None."""
        tmp = os.path.join(OUT, '_bake%04d.glb' % number)
        o = unreal.GLTFExportOptions()
        o.bake_material_inputs = unreal.GLTFMaterialBakeMode.SIMPLE
        o.texture_image_format = unreal.GLTFTextureImageFormat.PNG
        result = unreal.GLTFExporter.export_to_gltf(material, tmp, o, set())
        if result.errors or not os.path.exists(tmp):
            self.skip('', '', 'Material', '', 'unsupported', 'material bake failed: ' + '; '.join(map(str, result.errors)), path)
            return None
        j, data = glb_json(tmp)
        os.remove(tmp)
        mats = [x for x in j.get('materials', []) if x.get('name') == material.get_name()] or j.get('materials', [])[:1]
        if not mats:
            self.skip('', '', 'Material', '', 'unsupported', 'material bake wrote no material', path)
            return None
        gm = mats[0]
        n = struct.unpack_from('<I', data, 12)[0]
        bin_at = 20 + n + 8
        pbr = gm.get('pbrMetallicRoughness', {})
        slots = [('base_color', pbr.get('baseColorTexture'), pbr.get('baseColorFactor', [1, 1, 1, 1])),
                 ('orm', pbr.get('metallicRoughnessTexture'), None), ('normal', gm.get('normalTexture'), None),
                 ('emissive', gm.get('emissiveTexture'), (gm.get('emissiveFactor') or [0, 0, 0]) + [1])]
        out = {}
        for key, ref, factor in slots:
            tex_id = '%s:baked_%s' % (path, key)
            if ref is not None:
                img = j['images'][j['textures'][ref['index']]['source']]
                view = j['bufferViews'][img['bufferView']]
                body = data[bin_at + view.get('byteOffset', 0): bin_at + view.get('byteOffset', 0) + view['byteLength']]
                baked_textures[tex_id] = {'bytes': body, 'baked_from': path, 'srgb': key in ('base_color', 'emissive')}
                out[key] = tex_id
            elif factor is not None and (key == 'base_color' or any(factor[:3])):
                baked_textures[tex_id] = {'factor': factor, 'baked_from': path, 'srgb': True}
                out[key] = tex_id
        occlusion = gm.get('occlusionTexture')
        if occlusion and (not pbr.get('metallicRoughnessTexture') or occlusion['index'] != pbr['metallicRoughnessTexture']['index']):
            self.skip('', '', 'Material', '', 'unsupported', 'baked occlusion is a separate texture', path)
        return out

    def texture_row(self, tex, path, file_rel):
        task = unreal.AssetExportTask()
        task.object = tex
        task.filename = os.path.join(OUT, file_rel)
        task.automated = True
        task.prompt = False
        task.replace_identical = True
        if not unreal.Exporter.run_asset_export_task(task) or not os.path.exists(task.filename):
            raise Refusal('texture export failed: ' + path)
        w, h, depth = png_header(task.filename)
        comp = str(tex.get_editor_property('compression_settings')).split('.')[-1].split(':')[0].strip('<> ')
        name = {'TC_DEFAULT': 'default', 'TC_NORMALMAP': 'normalmap', 'TC_MASKS': 'masks', 'TC_GRAYSCALE': 'grayscale',
                'TC_HDR': 'hdr', 'TC_HDR_COMPRESSED': 'hdr', 'TC_ALPHA': 'alpha', 'TC_DISPLACEMENTMAP': 'displacementmap',
                'TC_VECTOR_DISPLACEMENTMAP': 'vectordisplacementmap', 'TC_BC7': 'bc7', 'TC_EDITOR_ICON': 'ui'}.get(comp, 'other')
        return {'id': path, 'file': file_rel, 'width': w, 'height': h, 'srgb': bool(tex.get_editor_property('srgb')),
                'compression': name, 'origin': origin_of(path)}

    def sound_row(self, wave, path, file_rel):
        task = unreal.AssetExportTask()
        task.object = wave
        task.filename = os.path.join(OUT, file_rel)
        task.automated = True
        task.prompt = False
        task.replace_identical = True
        if not unreal.Exporter.run_asset_export_task(task) or not os.path.exists(task.filename):
            raise Refusal('sound export failed: ' + path)
        channels, rate = wav_header(task.filename)
        return {'id': path, 'file': file_rel, 'seconds': num(wave.get_editor_property('duration')), 'channels': channels,
                'rate': rate, 'origin': origin_of(path)}

    def run(self):
        if not OUT or not os.path.isdir(OUT) or os.listdir(OUT):
            raise Refusal('AIMPORT_OUT must name an existing empty folder')
        if '-nullrhi' in unreal.SystemLibrary.get_command_line().lower():
            raise Refusal('material baking needs a renderer: run without -nullrhi (use -RenderOffscreen)')
        plan_sha = os.environ.get('AIMPORT_PLAN_SHA256', '')
        if len(plan_sha) != 64 or any(c not in '0123456789abcdef' for c in plan_sha):
            raise Refusal('AIMPORT_PLAN_SHA256 is not a plan\'s SHA-256 (64 lower-case hex): every bundle comes from a plan')
        if not self.checks or not hasattr(self.checks, 'read_convex') or not hasattr(self.checks, 'read_bones'):
            raise Refusal('the AethyrBridge plugin (UAethyrEditorChecks.ReadConvex, ReadBones) is not loaded')
        maps = self.map_list()
        if not maps:
            raise Refusal('no maps to export')
        log('maps %s' % ', '.join(maps))
        queue = list(maps)
        while queue:
            m = queue.pop(0)
            if m in self.maps:
                continue
            self.maps[m] = self.collect_map(m)
            r = self.maps[m]
            log('%s collected placements=%d lights=%d spots=%d instances=%d' % (m, len(r['placements']), len(r['lights']), len(r['spots']), len(r['instances'])))
            for inst in r['instances']:
                if inst['world'] not in self.maps:
                    queue.append(inst['world'])
        flat = {m: self.flatten(m, (m,)) for m in maps}
        # every asset of a carried class under /Game, plus what the maps reference elsewhere
        for a in self.assets_of('StaticMesh'):
            self.static_ids.add(str(a.package_name) + '.' + str(a.asset_name))
        source_rigs = os.environ.get('AIMPORT_SOURCE_RIGS') == '1'
        for a in self.assets_of('SkeletalMesh'):
            path = str(a.package_name) + '.' + str(a.asset_name)
            if source_rigs and path not in self.skeletal_ids:
                self.skip('', '', '/Script/Engine.SkeletalMesh', '', 'unsupported', 'unplaced mesh, caller imports rigs separately', path)
            else:
                self.skeletal_ids.add(path)
        for a in self.assets_of('AnimSequence'):
            path = str(a.package_name) + '.' + str(a.asset_name)
            if source_rigs:
                self.skip('', '', '/Script/Engine.AnimSequence', '', 'unsupported', 'caller imports animations separately', path)
            else:
                self.anim_ids.add(path)
        for a in self.assets_of('SoundWave'):
            self.sound_ids.add(str(a.package_name) + '.' + str(a.asset_name))
        for a in self.assets_of('Texture2D'):
            self.texture_ids.add(str(a.package_name) + '.' + str(a.asset_name))
        for cls in ('Material', 'MaterialInstanceConstant'):
            for a in self.assets_of(cls):
                self.material_ids.add(str(a.package_name) + '.' + str(a.asset_name))
        for cls, (reason, detail) in sorted(ASSET_SKIPS.items()):
            script = '/Script/Engine'
            if cls == 'MetaSoundSource':
                script = '/Script/MetasoundEngine'
            elif cls == 'NiagaraSystem':
                script = '/Script/Niagara'
            elif cls == 'LevelSequence':
                script = '/Script/LevelSequence'
            elif cls == 'WidgetBlueprint':
                script = '/Script/UMGEditor'
            elif cls == 'ControlRigBlueprint':
                script = '/Script/ControlRigDeveloper'
            elif cls in ('IKRigDefinition', 'IKRetargeter'):
                script = '/Script/IKRig'
            for a in self.assets_of(cls, script):
                self.skip('', '', script + '.' + cls, '', reason, detail, str(a.package_name) + '.' + str(a.asset_name))

        manifest = {'aimport': 1, 'source': {
            'route': 'editor', 'project': os.environ.get('AIMPORT_PROJECT') or unreal.SystemLibrary.get_game_name(),
            'revision': os.environ.get('AIMPORT_REVISION') or None, 'engine': unreal.SystemLibrary.get_engine_version().split('-')[0],
            'maps': maps, 'plan_sha256': os.environ['AIMPORT_PLAN_SHA256']}}
        for d in ('meshes', 'skeletal', 'anims', 'textures', 'sounds'):
            os.makedirs(os.path.join(OUT, d), exist_ok=True)
        meshes = []
        for k, path in enumerate(sorted(self.static_ids)):
            mesh = unreal.load_asset(path)
            if not isinstance(mesh, unreal.StaticMesh):
                raise Refusal('not a static mesh: ' + path)
            meshes.append(self.export_mesh(mesh, path, 'meshes/%04d.glb' % (k + 1)))
            log('mesh %d/%d %s' % (k + 1, len(self.static_ids), path))
        skeletal = []
        for k, path in enumerate(sorted(self.skeletal_ids)):
            skeletal.append(self.export_skeletal(unreal.load_asset(path), path, 'skeletal/%04d.glb' % (k + 1)))
        anims = []
        for k, path in enumerate(sorted(self.anim_ids)):
            anims.append(self.export_anim(unreal.load_asset(path), path, 'anims/%04d.glb' % (k + 1)))
            if (k + 1) % 20 == 0:
                log('anim %d/%d' % (k + 1, len(self.anim_ids)))
        skeletons = [self.skeleton_row(p) for p in sorted(self.skeleton_ids)]
        self.add_material_closure()
        materials = []
        baked_textures = {}
        for k, path in enumerate(sorted(self.material_ids)):
            row = self.material_row(path)
            m = unreal.load_asset(path)
            if not isinstance(m, unreal.MaterialInstance) or row['parent'] == path:
                baked = self.bake(m, path, k + 1, baked_textures)
                if baked:
                    row['baked'] = baked
            row = dict((key, row[key]) for key in ['id', 'parent', 'root', 'blend', 'two_sided', 'shading', 'textures', 'scalars', 'vectors', 'baked', 'origin'] if key in row)
            materials.append(row)
        textures = []
        ids = sorted(self.texture_ids | set(baked_textures))
        for k, path in enumerate(ids):
            file_rel = 'textures/%04d.png' % (k + 1)
            if path in baked_textures:
                b = baked_textures[path]
                if 'bytes' in b:
                    with open(os.path.join(OUT, file_rel), 'wb') as f:
                        f.write(b['bytes'])
                else:
                    png_1x1(os.path.join(OUT, file_rel), b['factor'])
                w, h, depth = png_header(os.path.join(OUT, file_rel))
                textures.append({'id': path, 'file': file_rel, 'width': w, 'height': h, 'srgb': b['srgb'], 'compression': 'default',
                                 'baked_from': b['baked_from'], 'origin': origin_of(b['baked_from'])})
            else:
                tex = unreal.load_asset(path)
                textures.append(self.texture_row(tex, path, file_rel))
        sounds = [self.sound_row(unreal.load_asset(p), p, 'sounds/%04d.wav' % (k + 1)) for k, p in enumerate(sorted(self.sound_ids))]

        map_rows = []
        for m in maps:
            map_rows.append(self.map_row(m, flat[m]))
        for d in ('meshes', 'skeletal', 'anims', 'textures', 'sounds', 'landscape'):
            if os.path.isdir(os.path.join(OUT, d)) and not os.listdir(os.path.join(OUT, d)):
                os.rmdir(os.path.join(OUT, d))
        manifest.update({'files': [], 'meshes': meshes, 'skeletal_meshes': skeletal, 'skeletons': skeletons, 'animations': anims,
                         'materials': materials, 'textures': textures, 'sounds': sounds, 'maps': map_rows})
        skipped = sorted(self.skipped, key=lambda r: (r['map'], r['actor'], r['component'], r['reason'], r.get('detail', ''), r['class']))
        with open(os.path.join(OUT, 'skipped.json'), 'w') as f:
            f.write(json.dumps(skipped, indent=1) + '\n')
        self.check_references(manifest)
        files = []
        for top, _, names in os.walk(OUT):
            for n in names:
                full = os.path.join(top, n)
                rel = os.path.relpath(full, OUT)
                if rel in ('manifest.json', 'skipped.json'):   # the bundle's own two files are not listed
                    continue
                with open(full, 'rb') as f:
                    data = f.read()
                files.append({'path': rel, 'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data)})
        manifest['files'] = sorted(files, key=lambda r: r['path'])
        tmp = os.path.join(OUT, '.manifest.tmp')
        with open(tmp, 'w') as f:
            f.write(json.dumps(manifest, indent=1) + '\n')
        os.rename(tmp, os.path.join(OUT, 'manifest.json'))
        self.report(manifest, skipped)

    def map_row(self, map_id, flat):
        for s in flat['skipped']:
            self.skip(map_id, s[0], s[1], s[2], s[3], s[4])
        placements = []
        order = ['name', 'actor', 'actor_class', 'component', 'instance', 'kind', 'asset', 'translation_cm', 'rotation', 'scale',
                 'materials', 'collision', 'visible', 'cast_shadow', 'tags']
        for r in flat['placements']:
            r = dict(r, scale=[1, 1, 1])
            put_trs(r, r.pop('_t'))
            placements.append(dict((k, r[k]) for k in order if k in r))
        lights = []
        light_layers = {}
        for r in flat['lights']:
            r = dict(r)
            put_trs(r, r.pop('_t'))
            if r.get('data_layers'):
                light_layers[r['name']] = r['data_layers']
            keys = r.pop('_order')
            lights.append(dict((k, r[k]) for k in keys if k in r))
        spots = []
        for r in flat['spots']:
            r = dict(r)
            r.pop('_t', None)
            spots.append(dict((k, r[k]) for k in ['name', 'kind', 'translation_cm', 'rotation', 'tags'] if k in r))
        volumes = []
        for r in flat['volumes']:
            r = dict(r)
            put_trs(r, r.pop('_t'))
            volumes.append(dict((k, r[k]) for k in ['name', 'class', 'translation_cm', 'rotation', 'scale', 'boxes'] if k in r))
        for key, rows in (('placements', placements), ('lights', lights), ('spots', spots), ('volumes', volumes)):
            names = [r['name'] for r in rows]
            if len(set(names)) != len(names):
                dup = sorted(n for n in set(names) if names.count(n) > 1)[0]
                raise Refusal('%s: %s name %s is not unique' % (map_id, key, dup))
        atmosphere = {'fog': {}, 'sky': {}, 'post': {}, 'all': [], 'light_data_layers': light_layers}
        for kind, actor, layers, values in sorted(flat['atmos'], key=lambda a: (a[0], a[1])):
            row = {'kind': kind, 'actor': actor, 'values': values}
            if layers:
                row['data_layers'] = layers
            atmosphere['all'].append(row)
        for kind in ('fog', 'sky_atmosphere', 'post', 'cloud'):
            first = [a for a in atmosphere['all'] if a['kind'] == kind and not a.get('data_layers')] or [a for a in atmosphere['all'] if a['kind'] == kind]
            if first:
                atmosphere[kind] = first[0]['values']
        sky = [l for l in lights if l['kind'] == 'sky']
        if sky:
            atmosphere['sky'] = {k: sky[0][k] for k in ('name', 'intensity', 'color', 'cast_shadows', 'movable')}
        return {'id': map_id, 'placements': sorted(placements, key=lambda r: r['name']), 'lights': sorted(lights, key=lambda r: r['name']),
                'spots': sorted(spots, key=lambda r: r['name']), 'volumes': sorted(volumes, key=lambda r: r['name']),
                'landscapes': [], 'atmosphere': atmosphere, 'streamed': sorted(flat['streamed'])}

    def check_references(self, manifest):
        known = {k: {r['id'] for r in manifest[k]} for k in ('meshes', 'skeletal_meshes', 'skeletons', 'animations', 'materials', 'textures', 'sounds')}
        def need(kind, ref, where):
            if ref is not None and ref not in known[kind]:
                raise Refusal('%s refers to %s %s, which the bundle does not hold' % (where, kind, ref))
        for r in manifest['meshes']:
            for m in r['materials']:
                need('materials', m, r['id'])
        for r in manifest['skeletal_meshes']:
            need('skeletons', r['skeleton'], r['id'])
            for m in r['materials']:
                need('materials', m, r['id'])
        for r in manifest['animations']:
            need('skeletons', r['skeleton'], r['id'])
        for r in manifest['materials']:
            need('materials', r['parent'], r['id'])
            for key in ('textures', 'scalars', 'vectors'):
                if len(r[key]) > 256:
                    raise Refusal('%s has %d %s, over 256' % (r['id'], len(r[key]), key))
            need('materials', r['root'], r['id'])
            for t in r['textures']:
                need('textures', t['texture'], r['id'])
            for t in r.get('baked', {}).values():
                need('textures', t, r['id'])
        for mp in manifest['maps']:
            for p in mp['placements']:
                need('meshes' if p['kind'] == 'static' else 'skeletal_meshes', p['asset'], p['name'])
                for m in p['materials']:
                    need('materials', m, p['name'])

    def report(self, manifest, skipped):
        mats = {r['id']: r for r in manifest['materials']}
        for mp in manifest['maps']:
            used_m = set()
            pending = [m for p in mp['placements'] for m in p['materials'] if m]
            while pending:
                m = pending.pop()
                if m in used_m:
                    continue
                used_m.add(m)
                if mats[m]['parent'] != m:
                    pending.append(mats[m]['parent'])
            used_t = {t['texture'] for m in used_m for t in mats[m]['textures'] if t['texture']} | {t for m in used_m for t in mats[m].get('baked', {}).values()}
            reasons = {}
            for s in skipped:
                if s['map'] == mp['id']:
                    reasons[s['reason']] = reasons.get(s['reason'], 0) + 1
            used = {p['asset'] for p in mp['placements']}
            log('COUNT map=%s placements=%d meshes=%d materials=%d textures=%d lights=%d spots=%d volumes=%d streamed=%d skipped=%s' % (
                mp['id'], len(mp['placements']), len(used), len(used_m), len(used_t), len(mp['lights']), len(mp['spots']), len(mp['volumes']),
                len(mp['streamed']), ','.join('%s:%d' % kv for kv in sorted(reasons.items())) or '0'))
        log('AIMPORT_DONE meshes=%d skeletal=%d skeletons=%d animations=%d materials=%d textures=%d sounds=%d maps=%d files=%d skipped=%d' % (
            len(manifest['meshes']), len(manifest['skeletal_meshes']), len(manifest['skeletons']), len(manifest['animations']),
            len(manifest['materials']), len(manifest['textures']), len(manifest['sounds']), len(manifest['maps']),
            len(manifest['files']), len(skipped)))


FOG_PROPS = ['fog_density', 'fog_height_falloff', 'fog_inscattering_luminance', 'fog_max_opacity', 'start_distance', 'end_distance',
             'fog_cutoff_distance', 'second_fog_data', 'directional_inscattering_exponent', 'directional_inscattering_start_distance',
             'directional_inscattering_luminance', 'volumetric_fog', 'volumetric_fog_scattering_distribution', 'volumetric_fog_albedo',
             'volumetric_fog_extinction_scale', 'volumetric_fog_distance', 'sky_atmosphere_ambient_contribution_color_scale']
SKY_ATMOSPHERE_PROPS = ['transform_mode', 'bottom_radius', 'ground_albedo', 'atmosphere_height', 'multi_scattering_factor',
                        'rayleigh_scattering_scale', 'rayleigh_scattering', 'rayleigh_exponential_distribution', 'mie_scattering_scale',
                        'mie_scattering', 'mie_absorption_scale', 'mie_absorption', 'mie_anisotropy', 'mie_exponential_distribution',
                        'other_absorption_scale', 'other_absorption', 'sky_luminance_factor', 'aerial_pespective_view_distance_scale',
                        'height_fog_contribution']
CLOUD_PROPS = ['layer_bottom_altitude', 'layer_height', 'tracing_start_max_distance', 'tracing_max_distance', 'planet_radius',
               'ground_albedo', 'sky_light_cloud_bottom_occlusion']


def plain(v):
    """An Unreal value as JSON: numbers, bools, strings, enums by name, lists, structs by field."""
    if isinstance(v, bool) or v is None:
        return v
    if isinstance(v, (int, float)):
        return num(v)
    if isinstance(v, str):
        return v
    if isinstance(v, unreal.EnumBase):
        return str(v).split('.')[-1].split(':')[0].strip('<> ').lower()
    if isinstance(v, unreal.StructBase):
        return {k: plain(x) for k, x in sorted(v.to_dict().items())}
    if isinstance(v, unreal.Object):
        return v.get_path_name()
    if isinstance(v, (list, tuple, unreal.Array)):
        return [plain(x) for x in v]
    return str(v)


try:
    Export().run()
except Exception as e:
    reason = str(e) if isinstance(e, Refusal) else traceback.format_exc().replace('\n', ' | ')
    unreal.log_error('AIMPORT_FAILED ' + reason)
