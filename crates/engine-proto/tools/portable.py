"""Asset-tool helpers for the versioned engine interchange (not admission)."""
import hashlib
import math
from pathlib import Path, PurePosixPath
import re

from openagents.engine.v1.scene_pb2 import AssetManifest, CompiledScene

MAX_SCENE_BYTES = 16 * 1024 * 1024


def identifier(value, limit=48):
    return bool(re.fullmatch(r'[A-Za-z0-9_-]{1,' + str(limit) + '}', value))


def text(value, required=False):
    return (not required or bool(value)) and len(value.encode('utf-8')) <= 192 and all(ord(c) >= 32 and ord(c) != 127 for c in value)


def point(value):
    return all(math.isfinite(n) and abs(n) <= 1_000_000 for n in (value.x, value.y, value.z))


def camera(value):
    return (identifier(value.id) and text(value.name) and value.HasField('position') and value.HasField('target')
            and point(value.position) and point(value.target) and value.position != value.target
            and math.isfinite(value.vertical_fov_degrees) and 1 <= value.vertical_fov_degrees <= 179)


def validate_scene(scene):
    if (scene.schema_version != 1 or not identifier(scene.scene_id) or scene.coordinates != 1
            or len(scene.placements) > 32768 or len(scene.lights) > 2048 or len(scene.cameras) > 64
            or not scene.HasField('spawn') or not camera(scene.spawn)):
        raise ValueError('Invalid compiled scene header, limits, or spawn')
    for values in (scene.placements, scene.lights, scene.cameras):
        ids = [v.id for v in values]
        if any(not identifier(v) for v in ids) or len(set(ids)) != len(ids):
            raise ValueError('Invalid or duplicate compiled scene ID')
    for p in scene.placements:
        m = p.transform
        if (not text(p.asset_id, True) or not text(p.actor) or not text(p.source_class) or len(m) != 16
                or any(not math.isfinite(n) or abs(n) > 1_000_000 for n in m)):
            raise ValueError('Invalid compiled placement')
        determinant = m[0]*(m[5]*m[10]-m[9]*m[6])-m[4]*(m[1]*m[10]-m[9]*m[2])+m[8]*(m[1]*m[6]-m[5]*m[2])
        if abs(determinant) <= 1e-12 or any(abs(m[i]) > 1e-5 for i in (3, 7, 11)) or abs(m[15]-1) > 1e-5:
            raise ValueError('Placement transform must be nonsingular and affine')
    for light in scene.lights:
        if (not light.HasField('position') or not point(light.position) or not light.HasField('linear_color')
                or any(not math.isfinite(n) or not 0 <= n <= 32 for n in (light.linear_color.x, light.linear_color.y, light.linear_color.z))
                or not math.isfinite(light.radius_m) or not 0 < light.radius_m <= 1_000_000):
            raise ValueError('Invalid compiled light')
    if any(not camera(c) for c in scene.cameras):
        raise ValueError('Invalid compiled camera')
    return scene


def decode_scene(data):
    if len(data) > MAX_SCENE_BYTES:
        raise ValueError('Compiled scene exceeds byte limit')
    return validate_scene(CompiledScene.FromString(data))


def safe_path(value):
    path = PurePosixPath(value)
    return (text(value, True) and not path.is_absolute() and str(path) == value
            and all(p not in ('', '.', '..') for p in value.split('/')) and '\\' not in value and ':' not in value)


def validate_manifest(manifest):
    if manifest.schema_version != 1 or not identifier(manifest.scene_id) or not identifier(manifest.profile, 64) or not 1 <= len(manifest.artifacts) <= 32:
        raise ValueError('Invalid asset manifest header or limits')
    ids, paths, total = set(), set(), 0
    for artifact in manifest.artifacts:
        if (not identifier(artifact.id) or artifact.id in ids or not safe_path(artifact.path) or artifact.path in paths
                or len(artifact.sha256) != 32 or not 0 < artifact.byte_length <= 512*1024*1024 or not identifier(artifact.format, 64)
                or len(artifact.dependencies) > 8 or len(set(artifact.dependencies)) != len(artifact.dependencies) or len(artifact.buffers) > 8):
            raise ValueError('Invalid asset artifact')
        ids.add(artifact.id)
        paths.add(artifact.path)
        total += artifact.byte_length
        semantics = set()
        for buffer in artifact.buffers:
            if (not identifier(buffer.semantic) or buffer.semantic in semantics or not buffer.byte_length or not buffer.byte_stride
                    or buffer.byte_offset + buffer.byte_length > artifact.byte_length
                    or buffer.element_count * buffer.byte_stride != buffer.byte_length):
                raise ValueError('Invalid asset buffer descriptor')
            semantics.add(buffer.semantic)
    if total > 1024*1024*1024:
        raise ValueError('Asset manifest exceeds total byte limit')
    graph = {a.id: list(a.dependencies) for a in manifest.artifacts}
    active, done = set(), set()
    def visit(key):
        if key not in graph or key in active:
            raise ValueError('Missing or cyclic asset dependency')
        if key in done:
            return
        active.add(key)
        for dependency in graph[key]:
            visit(dependency)
        active.remove(key)
        done.add(key)
    for key in graph:
        visit(key)
    return manifest


def checked_file(root, relative, expected=None):
    if not safe_path(relative):
        raise ValueError('Unsafe asset path')
    root = Path(root).resolve()
    current = root
    for part in relative.split('/'):
        current = current / part
        if current.is_symlink():
            raise ValueError('Symlink asset path')
    size = current.stat().st_size
    if not 0 < size <= 512*1024*1024 or (expected is not None and size != expected):
        raise ValueError('Asset length mismatch')
    digest = hashlib.sha256()
    with current.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024*1024), b''):
            digest.update(chunk)
    return size, digest.digest()


def verify_files(manifest, root):
    validate_manifest(manifest)
    for artifact in manifest.artifacts:
        size, digest = checked_file(root, artifact.path, artifact.byte_length)
        if digest != artifact.sha256:
            raise ValueError('Asset hash mismatch: ' + artifact.path)
    return manifest


def manifest_for(root, scene_id, profile, entries):
    manifest = AssetManifest(schema_version=1, scene_id=scene_id, profile=profile)
    for identity, path, format, dependencies in entries:
        size, digest = checked_file(root, path)
        manifest.artifacts.add(id=identity, path=path, format=format, dependencies=dependencies, byte_length=size, sha256=digest)
    return validate_manifest(manifest)
