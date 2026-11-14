# Firm reason: 6 (the Unreal Editor's material API exists only in Python and C++).
# The look's shader source and the two helpers that turn it into Unreal materials, shared by the
# importer (import.py: the generated M_AeCel and M_AeSky) and the look kit (look.py: a project's own
# copies with cast shadows and glow). Run inside the Unreal Editor's Python.
import re

import unreal

mel = unreal.MaterialEditingLibrary

AE_HELPERS = """
struct AeFn {
  float soft(float e, float s, float x) { float t = saturate((x - (e - s)) / max(2.0 * s, 0.000001)); return t * t * (3.0 - 2.0 * t); }
  float luma(float3 c) { return dot(c, float3(0.2126, 0.7152, 0.0722)); }
  float3 shade(float3 lt, float3 ti, float depth) {
    float3 s = lt * depth * max(ti, 0.000001) / max(luma(ti), 0.000001);
    return s * (luma(lt) * depth) / max(luma(s), 0.000001);
  }
  float keep(float3 c, float warm, float cyan) {
    float mx = max(c.r, max(c.g, c.b)); float mn = min(c.r, min(c.g, c.b)); float span = mx - mn;
    if (span < 0.0001) return 0.0;
    float lo = 1.0;
    if (c.b <= c.g && c.r >= c.g * 0.85 && (c.g - c.b) > span * 0.15) lo = warm;
    float gb = min(c.g, c.b);
    if (c.r <= gb && (gb - c.r) > span * 0.6) lo = cyan;
    float t = saturate((span / mx - lo) / 0.1);
    return t * t * (3.0 - 2.0 * t);
  }
  float3 gray(float3 c, float warm, float cyan) { float g = luma(c); return lerp(float3(g, g, g), c, keep(c, warm, cyan)); }
  float3 enc(float3 c) { c = max(c, 0.0); return lerp(c * 12.92, 1.055 * pow(c, 1.0 / 2.4) - 0.055, step(0.0031308, c)); }
  float3 dec(float3 c) { c = max(c, 0.0); return lerp(c / 12.92, pow((c + 0.055) / 1.055, 2.4), step(0.04045, c)); }
  float3 post(float3 c, float4 tn, bool gr) {
    c = 1.0 - exp(-max(c, 0.0) * tn.x);
    float l = luma(c);
    c = max(0.0, l + (c - l) * tn.y);
    c = tn.z + (1.0 - tn.z) * c;
    float3 g = pow(saturate(c), 1.0 / 2.2);
    if (gr) g = gray(g, 0.62, 0.38);
    return dec(g);
  }
};
AeFn ae;
"""
AE_CEL = AE_HELPERS + """
float3 w = float3(WP.x, WP.z, WP.y) * 0.01;
float3 n = normalize(float3(NW.x, NW.z, NW.y) + float3(0.0, 0.00001, 0.0));
bool gr = FX.z < -0.5;
float3 srgb = ae.enc(VC.rgb);
if (gr) srgb = ae.gray(srgb, 0.75, 0.5);
float3 albedo = pow(max(srgb, 0.0), 2.2);
float mat = UVB.x; float param = UVA.x;
float3 tov = EYE.xyz - w; float dist = length(tov); float3 v = tov / max(dist, 0.0001);
if (dot(n, v) < 0.0) n = -n;
float3 c;
if (mat > 2.5 && mat < 3.5) { c = albedo * param; }
else if ((mat > 3.5 && mat < 4.5) || (mat > 6.5 && mat < 7.5)) { c = albedo; if (mat < 4.5) c *= 0.55 + 0.45 * S0.w; }
else if (mat > 8.5 && mat < 9.5) { c = albedo * param; }
else {
  if (mat > 4.5 && mat < 5.5) {
    float2 g = float2(w.x * 0.62, w.z * 0.62 + 0.5 * floor(w.x * 0.62)); float2 f = frac(g);
    float joint = min(min(f.x, 1.0 - f.x), min(f.y, 1.0 - f.y));
    float stone = frac(sin(dot(floor(g), float2(12.9898, 78.233))) * 43758.5453);
    albedo *= (0.9 + 0.16 * stone) * (joint < 0.035 ? 0.62 : 1.0);
  }
  float3 key = normalize(S1.xyz);
  float ndl = dot(n, key); float ndv = dot(n, v);
  float wgt = saturate(ndl * 0.5 + 0.5);
  float share = S9.z * (1.0 - exp(-S9.y * max(dist - S9.x, 0.0)));
  float aa = S7.z * (1.0 + 8.0 * saturate(share));
  bool chr = (mat > 0.5 && mat < 2.5) || (mat > 9.5 && mat < 10.5);
  float tone = chr ? ae.soft(S8.x - 0.12 * saturate(ndv), max(S8.y, 0.000001), wgt) : ae.soft(S7.x, max(aa, 0.000001), wgt);
  if (mat > 5.5 && mat < 6.5) tone = lerp(1.0, tone, DRW.y);
  float3 hi = albedo * (S0.xyz * S0.w + S5.xyz * S5.w);
  float3 lo = ae.shade(hi, chr ? float3(1.0, 0.80, 0.82) : S4.xyz, S7.w);
  c = lerp(lo, hi, tone);
#define AE_LIGHT(P, C) if (C.w > 0.0) { float3 dl = P.xyz - w; float dd = length(dl); float fall = saturate(1.0 - dd / max(P.w, 0.01)); c += albedo * C.xyz * C.w * ae.soft(0.18, 0.02, saturate(dot(n, dl / max(dd, 0.0001)) * 0.5 + 0.5) * fall * fall); }
  AE_LIGHT(L0, L1) AE_LIGHT(L2, L3) AE_LIGHT(L4, L5) AE_LIGHT(L6, L7) AE_LIGHT(L8, L9) AE_LIGHT(L10, L11) AE_LIGHT(L12, L13) AE_LIGHT(L14, L15)
#undef AE_LIGHT
  if (mat > 1.5 && mat < 2.5) { float3 hh = normalize(key + v); c += (hi + float3(0.2, 0.2, 0.25)) * ae.soft(S8.z - 0.25, 0.02, abs(dot(n, hh))) * S8.w * 0.5; }
  float strength = chr ? S3.x : S2.w * 0.5;
  float edge = 1.0 - ae.soft((S3.y + S3.z) * 0.5, max((S3.y - S3.z) * 0.5, 0.000001), abs(ndv));
  c += S2.xyz * edge * (0.2 + 0.8 * saturate(ndl + 0.35)) * strength;
  c = lerp(c, S10.xyz, share);
}
return ae.post(c, S18, gr);
"""
AE_SKY = AE_HELPERS + """
float3 w = float3(WP.x, WP.z, WP.y) * 0.01;
float3 d = normalize(w - EYE.xyz);
bool gr = FX.z < -0.5;
float up = d.y;
float3 sky = S13.xyz + (S12.xyz - S13.xyz) * pow(max(saturate(up), 0.000001), S13.w);
float3 sun = normalize(S1.xyz); if (S1.w > 0.5) sun = -sun;
float cs = (d.x * sun.x + d.z * sun.z) / (sqrt(max(d.x * d.x + d.z * d.z, 0.000001)) * sqrt(max(sun.x * sun.x + sun.z * sun.z, 0.000001)));
float side = saturate(cs * 0.5 + 0.5);
float h = max(S12.w, 0.001) * (0.6 + 0.4 * side);
sky = lerp(sky, S11.xyz, (1.0 - ae.soft(h * 0.5, h * 0.5, max(up, 0.0))) * (0.33 + 0.67 * side) * S11.w);
float m = dot(d, normalize(float3(-0.62, 0.36, -0.70))); float ms = MISC.w;
sky += float3(1.0, 0.86, 0.62) * ms * (0.25 * exp(-120.0 * (1.0 - m)));
sky = lerp(sky, float3(1.6, 1.45, 1.15), ae.soft(0.9978, 0.00025, m) * ms);
return ae.post(sky, S18, gr);
"""


# The style rows a material reads, by the name its code uses: where each sits in the uniform block.
def named_uniforms(manifest, slots=(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 18, 31)):
    u = manifest["uniforms"]
    named = {"EYE": u["eye"], "FX": u["fx"], "DRW": u["draw"], "MISC": u["misc"]}
    for slot in slots:
        named["S%d" % slot] = u["style"] + slot * 4
    for k in range(u["light_count"] * 2):
        named["L%d" % k] = u["lights"] + k * 4
    return named


# The collection's vector parameters: U<n> for each offset (the bridge copies floats n to n + 3 in
# each frame), then `extra` (name: default) for a project's own.
def set_collection(collection, named, extra=None):
    params = []
    for offset in sorted(set(named.values())):
        param = unreal.CollectionVectorParameter()
        param.set_editor_property("parameter_name", "U%d" % offset)
        param.set_editor_property("default_value", unreal.LinearColor(0, 0, 0, 0))
        params.append(param)
    for name, value in sorted((extra or {}).items()):
        param = unreal.CollectionVectorParameter()
        param.set_editor_property("parameter_name", name)
        param.set_editor_property("default_value", unreal.LinearColor(*value))
        params.append(param)
    collection.set_editor_property("vector_parameters", params)


# One custom node as a material's whole colour. `geometry` is the node's inputs that are
# expressions (name: class, or name: a function making the expression); every uniform name the code
# uses (named, or `extra` name: parameter name) becomes an input from the collection.
def custom_material(mat, code, geometry, named, collection, extra=None, unlit=True):
    mel.delete_all_material_expressions(mat)
    if unlit:
        mat.set_editor_property("shading_model", unreal.MaterialShadingModel.MSM_UNLIT)
        mat.set_editor_property("two_sided", True)
    node = mel.create_material_expression(mat, unreal.MaterialExpressionCustom, -300, 0)
    node.set_editor_property("code", code)
    node.set_editor_property("output_type", unreal.CustomMaterialOutputType.CMOT_FLOAT3)
    params = {k: "U%d" % v for k, v in named.items()}
    params.update(extra or {})
    used = [k for k in params if re.search(r"\b%s\b" % k, code)]
    entries = []
    for key in list(geometry) + used:
        entry = unreal.CustomInput()
        entry.set_editor_property("input_name", key)
        entries.append(entry)
    node.set_editor_property("inputs", entries)
    y = -800
    for key, cls in geometry.items():
        if isinstance(cls, type):
            src = mel.create_material_expression(mat, cls, -900, y)
            if key == "UVB":
                src.set_editor_property("coordinate_index", 1)
            mel.connect_material_expressions(src, "", node, key)
        else:
            src, pin = cls(mat, -900, y)
            mel.connect_material_expressions(src, pin, node, key)
        y += 120
    for key in used:
        src = mel.create_material_expression(mat, unreal.MaterialExpressionCollectionParameter, -1300, y)
        src.set_editor_property("collection", collection)
        src.set_editor_property("parameter_name", params[key])
        mel.connect_material_expressions(src, "", node, key)
        y += 60
    mel.connect_material_property(node, "", unreal.MaterialProperty.MP_EMISSIVE_COLOR)
    mel.recompile_material(mat)
    return mat
