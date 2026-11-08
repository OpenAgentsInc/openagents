# Firm reason: 2 (independent Node oracle of the browser's optional FX binding transport).
"""Legacy FX defaults and strict additive opt-in; no game/UI policy in the host."""
from pathlib import Path
import re
import subprocess

repo = Path(__file__).resolve().parents[3]
source = (repo / 'coax/web/host.js').read_text()
match = re.search(r'function fxSceneOptions\(x, scene, gpui\) \{[\s\S]*?\n\}', source)
assert match, 'missing optional platform binding adapter'
script = match.group(0) + r'''
const assert = require('node:assert/strict');
assert.deepEqual(fxSceneOptions(undefined, true, false), {scene:false, alpha:false});
assert.deepEqual(fxSceneOptions({}, true, false), {scene:false, alpha:false});
assert.deepEqual(fxSceneOptions({}, true, true), {scene:false, alpha:true});
const opted = {coaxial_fx_scene_v1:()=>1, coaxial_fx_alpha_v1:()=>1};
assert.deepEqual(fxSceneOptions(opted, true, false), {scene:true, alpha:true});
assert.deepEqual(fxSceneOptions(opted, false, false), {scene:false, alpha:true});
assert.deepEqual(fxSceneOptions({coaxial_fx_scene_v1:()=>2,coaxial_fx_alpha_v1:()=>2},true,false), {scene:false,alpha:false});
console.log('FX source atlas and alpha: legacy defaults, GPUI, strict opt-in and absent-scene gates passed');
'''
subprocess.run(['node', '-e', script], check=True, cwd=repo)
