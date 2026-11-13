# Firm reason: 2 (independent row padding, base64, logical string-budget and compiler-input extent reconstruction from actual host artifacts).
import hashlib
import json
from pathlib import Path
import sys


def inspect(site, receipt):
    r = json.loads(receipt.read_text())
    assert set(r['facts']) == {'compileInputsZeroed', 'joinRetainsOwners', 'encoderFailure', 'preAllocationPressure', 'retryPixels', 'sourceExtents', 'cleanup'}
    assert all(r['facts'].values()) and r['frameCount'] == 10
    width, height, row = r['size']
    assert (width, height) == (320, 180) and row == (width*4+255)//256*256
    raw = row*height;encoded = (raw+2)//3*4
    budget = 2*max(2*raw+32768, raw+encoded)
    c = r['capture'];assert c['schema'] == 'aethyr.stream-capture-cost.v1'
    assert c['copyBytes'] == raw and c['encodedUnits'] == encoded
    assert c['textBudgetBytes'] == budget and c['observedLogicalUTF16Peak'] == 2*(raw+encoded)
    assert c['resultLogicalUTF16Bytes'] == 2*encoded
    baseline = r['baseline'];assert baseline == [3, 17170432, 268435456, 67108864]
    copy_reserved = (raw+65535)//65536*65536
    needed = 268435456-baseline[1]-copy_reserved
    assert r['pressured'] == [baseline[0]+(needed+67108863)//67108864, baseline[1]+needed, *baseline[2:]]
    assert r['cleanup'] == [1, 8388608, *baseline[2:]]
    a = r['artifacts'];assert a['schema'] == 'aethyr.stream-host-artifacts.v1'
    engine = (site/'build/stream-walk.wasm').read_bytes()
    assert a['engineInputBytes'] == len(engine) and a['engineSHA'] == hashlib.sha256(engine).hexdigest() and a['engineInputZeroed']
    shaders = [(site/'build'/name).read_text() for name in ('stream-draw.wgsl', 'stream-hud.wgsl', 'stream-present.wgsl')]
    units = [len(s.encode('utf-16-le'))//2 for s in shaders]
    assert a['shaderCodeUnits'] == units and a['shaderLogicalUTF16Bytes'] == 2*sum(units)
    compiler = r['readerCompiler'];assert compiler['schema'] == 'aethyr.pack-compiler-input.v1' and compiler['sourceCleared']
    assert compiler['sourceBytes'] == (site/'coax/build/host-packs.wasm').stat().st_size
    assert len(r['pixelSHA']) == 64 and r['pixelSHA'] != hashlib.sha256(bytes(raw)).hexdigest()
    return {'schema': 'aethyr.capture-cost-oracle.v1', 'source_bytes': raw, 'base64_units': encoded, 'copy_reserved': copy_reserved, 'text_reserved': (budget+65535)//65536*65536, 'logical_text_peak': 2*(raw+encoded), 'engine_input_bytes': len(engine), 'shader_utf16_bytes': 2*sum(units), 'failures_clean': True, 'sdk_gc_driver_physical_bytes': 'not measured; separate from explicit reservations'}


if __name__ == '__main__':
    print(json.dumps(inspect(Path(sys.argv[1]), Path(sys.argv[2])), indent=2))
