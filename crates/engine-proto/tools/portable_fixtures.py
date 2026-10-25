"""Synthetic interchange corpus shared by Rust, Python, and C++ consumers."""
import json


def generate(root):
    from openagents.engine.v1.scene_pb2 import CompiledScene
    from portable import manifest_for
    folder = root / 'fixtures/portable'
    folder.mkdir(parents=True)
    (folder / 'buffer.bin').write_bytes(bytes(range(48)))
    scene = CompiledScene(schema_version=1, scene_id='town-fixture', coordinates=1)
    scene.spawn.id, scene.spawn.vertical_fov_degrees = 'spawn', 60
    scene.spawn.position.y, scene.spawn.target.z = 2, 4
    scene.spawn.name = 'Entry'
    scene.cameras.add().CopyFrom(scene.spawn)
    placement = scene.placements.add(id='piece-1', asset_id='owned/cube', actor='House', source_class='Owned')
    placement.transform.extend([1,0,0,0, 0,1,0,0, 0,0,1,0, 2,0,3,1])
    light = scene.lights.add(id='lamp', radius_m=6)
    light.position.y = 3
    light.linear_color.x = 1
    (folder / 'scene.pb').write_bytes(scene.SerializeToString(deterministic=True))
    manifest = manifest_for(folder, 'town-fixture', 'fixture-v1', [('geometry','buffer.bin','float32-v1',[]),('scene','scene.pb','CompiledScene-v1',['geometry'])])
    manifest.artifacts[0].buffers.add(semantic='positions', byte_offset=0, byte_length=48, byte_stride=12, element_count=4)
    cases = {}
    def write(name, value, valid=True):
        (folder / name).write_bytes(value.SerializeToString(deterministic=True))
        cases[name] = valid
    write('manifest.pb', manifest)
    cases['scene.pb'] = True
    precision=CompiledScene();precision.CopyFrom(scene)
    precision.placements[0].transform[:]=[1,1.0000001192092896,0,0,1.0000001192092896,1.000000238418579,0,0,0,0,1_000_000,0,0,0,0,1]
    write('scene-precision.pb',precision)
    for name, value in [('scene-unknown.pb',scene),('manifest-unknown.pb',manifest)]:
        (folder / name).write_bytes(value.SerializeToString(deterministic=True) + b'\x98\x06\x01')
        cases[name] = True
    def invalid(name, base, mutate):
        value = type(base)(); value.CopyFrom(base); mutate(value)
        write(name, value, False)
    invalid('scene-invalid-transform.pb',scene,lambda v:v.placements[0].transform.pop())
    invalid('scene-invalid-singular.pb',scene,lambda v:v.placements[0].transform.__setitem__(0,0))
    invalid('scene-invalid-projective.pb',scene,lambda v:v.placements[0].transform.__setitem__(3,1))
    invalid('scene-invalid-spawn.pb',scene,lambda v:v.ClearField('spawn'))
    invalid('scene-invalid-camera.pb',scene,lambda v:setattr(v.cameras[0],'vertical_fov_degrees',180))
    invalid('scene-invalid-color.pb',scene,lambda v:setattr(v.lights[0].linear_color,'x',-1))
    invalid('scene-invalid-utftext.pb',scene,lambda v:setattr(v.placements[0],'actor','A\0B'))
    invalid('scene-invalid-duplicate.pb',scene,lambda v:v.placements.add().CopyFrom(v.placements[0]))
    def excess_lights(v):
        while len(v.lights) < 2049:
            l = v.lights.add(); l.CopyFrom(light); l.id='lamp-'+str(len(v.lights))
    invalid('scene-invalid-count.pb',scene,excess_lights)
    invalid('manifest-invalid-path.pb',manifest,lambda v:setattr(v.artifacts[0],'path','../buffer.bin'))
    invalid('manifest-invalid-digest.pb',manifest,lambda v:setattr(v.artifacts[0],'sha256',b'bad'))
    invalid('manifest-invalid-cycle.pb',manifest,lambda v:v.artifacts[0].dependencies.append('scene'))
    invalid('manifest-invalid-missing.pb',manifest,lambda v:v.artifacts[0].dependencies.append('missing'))
    invalid('manifest-invalid-buffer.pb',manifest,lambda v:setattr(v.artifacts[0].buffers[0],'element_count',5))
    invalid('manifest-invalid-overflow.pb',manifest,lambda v:setattr(v.artifacts[0].buffers[0],'byte_offset',2**64-1))
    (folder / 'cases.json').write_text(json.dumps(cases, indent=2)+'\n')
