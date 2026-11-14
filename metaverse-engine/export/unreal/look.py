# Firm reason: 6 (the Unreal Editor's asset and level API exists only in Python and C++).
# A starter look for an imported game, made where an import never writes: /Game/<Game>/Overrides/Look/.
# It stands in for an artist. Everything here is Unreal's side of the picture (README.md, "A look of
# your own"): cast shadows from the key light, glow that blooms, height fog, a colour grade for each
# time of day, inner ink lines, and a sky with a sun disc and stars. The game's rules and the
# generated content are not touched; run it once, then change the assets by hand or run it again.
#
#   AETHYR_BUNDLE=/abs/game.aexport UnrealEditor-Cmd Project.uproject \
#     -ExecutePythonScript=/abs/look.py -unattended -nosplash -RenderOffscreen
#
# It makes, in /Game/<Game>/Overrides/Look/:
#   MPC_Look        the numbers: the game's uniform block (the bridge writes it each frame) and the
#                   look's own dials (AeGlow, AeFog, AeInk, AeGrade..., AeSunDisc, AeShadowSet)
#   RT_LookShadow   the key light's depth picture (the bridge draws it each frame)
#   M_LookCel       the generated cel material with the cast shadow folded into its two tones, and
#                   glow written brighter than white so bloom finds it
#   M_LookSky       the generated sky with the sun as a pale hard-edged disc, and stars
#   M_LookPost      a post-process pass: inner ink lines, height fog, the grade, the vignette
#   L_Look          a level: the post-process volume (bloom, no film curve), a directional light that
#                   follows the key light and eight point lights that follow the game's lights
# and adds the rows look.level, look.params, look.shadow, material.cel and material.sky to
# DT_AethyrOverrides, leaving its other rows as they are. Import again afterwards: the importer puts
# material.cel and material.sky on the generated meshes.
import json
import os
import sys

import unreal

sys.path.insert(0, os.path.dirname(os.path.realpath(__file__)))
import look_source

manifest = json.load(open(os.path.join(os.environ["AETHYR_BUNDLE"], "manifest.json")))
game = manifest["game"]["name"]
OVERRIDES = "/Game/%s/Overrides" % game
LOOK = OVERRIDES + "/Look"
TABLE = OVERRIDES + "/DT_AethyrOverrides"
SHADOW_TEXELS = int(os.environ.get("AETHYR_LOOK_SHADOW_TEXELS", "2048"))

assets = unreal.EditorAssetLibrary
tools = unreal.AssetToolsHelpers.get_asset_tools()
levels = unreal.get_editor_subsystem(unreal.LevelEditorSubsystem)
actors = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
mel = unreal.MaterialEditingLibrary
lib = unreal.DataTableFunctionLibrary


def asset(name, cls, factory):
    path = "%s/%s" % (LOOK, name)
    if assets.does_asset_exist(path):
        return assets.load_asset(path)
    return tools.create_asset(name, LOOK, cls, factory)


def must_replace(code, old, new):
    if old not in code:
        raise SystemExit("look.py: the generated look no longer has the line: " + old)
    return code.replace(old, new, 1)


# ---- the numbers. The dials' values here are the defaults; change them in MPC_Look.
DIALS = {
    # the bridge writes these as the game runs (their values here are what a material sees in the editor)
    "AeLook": (0, 0.5, 0, 0),             # 1 while the game runs, the clock (0 midnight, 0.5 noon), seconds
    "AeSun": (0, 0, 1, 0),
    "AeShadowU": (0, 0, 0, 0), "AeShadowV": (0, 0, 0, 0), "AeShadowD": (0, 0, 0, 0), "AeShadowInfo": (0, 0, 0, 0),
    # the look's own
    "AeShadowSet": (110.0, 600.0, 0, 0),  # the shadow picture: metres of ground across, metres of depth
    "AeGlow": (2.0, 0, 0, 0),             # how far past white a glow is written (what blooms)
    "AeFog": (0.0045, 9.0, 0.0, 0.6),     # density a metre at the base height, the height it thins over (m), base height (m), the most it covers
    "AeFog2": (18.0, 0.0, 1100.0, 0),     # metres before the fog starts, the flat steps it is cut into (0: smooth), metres past which is sky
    "AeInk": (0.0009, 0.0006, 0.85, 90.0),  # the crease that starts a line, how soon it is full, its strength, metres it fades out by
    "AeSunDisc": (0.075, 0.0012, 0.9, 0.0),  # the sun's radius (rad), its edge, how pale, its halo (none: a flat disc)
    # the grade for each time of day: gain rgb and saturation, then a tint for the darks rgb and contrast
    "AeGradeNightGain": (0.86, 0.93, 1.12, 0.96), "AeGradeNightLift": (0.012, 0.014, 0.05, 0.10),
    "AeGradeDawnGain": (1.05, 0.98, 0.95, 1.04), "AeGradeDawnLift": (0.035, 0.018, 0.03, 0.08),
    "AeGradeDayGain": (1.03, 1.0, 0.97, 1.10), "AeGradeDayLift": (0.012, 0.008, 0.03, 0.12),
    "AeGradeDuskGain": (1.07, 0.95, 1.05, 1.12), "AeGradeDuskLift": (0.04, 0.012, 0.06, 0.10),
}
named = look_source.named_uniforms(manifest, slots=(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 18, 19, 31))
collection = asset("MPC_Look", unreal.MaterialParameterCollection, unreal.MaterialParameterCollectionFactoryNew())
look_source.set_collection(collection, named, DIALS)

shadow = asset("RT_LookShadow", unreal.TextureRenderTarget2D, unreal.TextureRenderTargetFactoryNew())
shadow.set_editor_property("render_target_format", unreal.TextureRenderTargetFormat.RTF_R32F)
shadow.set_editor_property("size_x", SHADOW_TEXELS)
shadow.set_editor_property("size_y", SHADOW_TEXELS)
shadow.set_editor_property("clear_color", unreal.LinearColor(1.0e9, 1.0e9, 1.0e9, 1.0))
shadow.set_editor_property("address_x", unreal.TextureAddress.TA_CLAMP)
shadow.set_editor_property("address_y", unreal.TextureAddress.TA_CLAMP)
shadow.set_editor_property("filter", unreal.TextureFilter.TF_NEAREST)


def shadow_texture(mat, x, y):
    node = mel.create_material_expression(mat, unreal.MaterialExpressionTextureObject, x, y)
    node.set_editor_property("texture", shadow)
    node.set_editor_property("sampler_type", unreal.MaterialSamplerType.SAMPLERTYPE_LINEAR_COLOR)
    return node, ""


# ---- the cel material: the generated one, with two changes
# The cast shadow: how lit a place is by the key light, from the bridge's depth picture, four
# taps a texel apart (crisp), multiplied into the light the two tones are cut from, as the
# prototype's own shadow map is (animelab.coax al_lit). A place in shadow takes the shadow tone.
SHADOW = """
  float lit = 1.0;
  if (SHI.x > 0.5 && ndl > 0.0) {
    float3 nu = float3(n.x, n.z, n.y);
    float texw = SHI.z * SHI.y;
    float3 sp = WPU.xyz + nu * (texw * 1.5 + 1.0);
    float2 suv = float2(dot(SHU.xyz, sp) + SHU.w, dot(SHV.xyz, sp) + SHV.w);
    float sd = dot(SHD.xyz, sp) + SHD.w;
    float rim = min(min(suv.x, suv.y), min(1.0 - suv.x, 1.0 - suv.y));
    if (rim > 0.0) {
      float2 st = suv / SHI.y - 0.5; float2 f = frac(st); float2 b = (floor(st) + 0.5) * SHI.y;
      float bias = 3.0 + texw * min(sqrt(max(1.0 - ndl * ndl, 0.0)) / max(ndl, 0.05), 4.0) * 0.6;
      float a0 = step(sd - bias, Texture2DSampleLevel(SH, SHSampler, b, 0).r);
      float a1 = step(sd - bias, Texture2DSampleLevel(SH, SHSampler, b + float2(SHI.y, 0.0), 0).r);
      float a2 = step(sd - bias, Texture2DSampleLevel(SH, SHSampler, b + float2(0.0, SHI.y), 0).r);
      float a3 = step(sd - bias, Texture2DSampleLevel(SH, SHSampler, b + float2(SHI.y, SHI.y), 0).r);
      lit = lerp(1.0, ae.soft(0.5, 0.2, lerp(lerp(a0, a1, f.x), lerp(a2, a3, f.x), f.y)), saturate(rim / 0.04));
    }
  }
  float wgt = saturate(ndl * 0.5 + 0.5) * lit;
"""
# Glow: the prototype blooms a glow material whole, a window by 0.6, and anything else by how far
# its light passes the style's bloom threshold. Here that share is written past white, for Unreal's
# bloom (threshold 1) to find. Flat marks and ink strokes never glow.
GLOW = """
float3 o = ae.post(c, S18, gr);
if (LK.x > 0.5) {
  float gl = 0.0;
  if (mat > 2.5 && mat < 3.5) gl = 1.0;
  else if (mat > 8.5 && mat < 9.5) gl = 0.6;
  else if (!((mat > 3.5 && mat < 4.5) || (mat > 6.5 && mat < 7.5))) gl = saturate((ae.luma(c) - S19.x + S19.y) / max(2.0 * S19.y, 0.0001));
  o *= 1.0 + GLW.x * gl;
}
return o;
"""
cel_code = must_replace(look_source.AE_CEL, "  float wgt = saturate(ndl * 0.5 + 0.5);\n", SHADOW)
cel_code = must_replace(cel_code, "return ae.post(c, S18, gr);\n", GLOW)
EXTRA = {"SHU": "AeShadowU", "SHV": "AeShadowV", "SHD": "AeShadowD", "SHI": "AeShadowInfo", "LK": "AeLook", "GLW": "AeGlow", "SUN": "AeSun",
         "FOG": "AeFog", "FG2": "AeFog2", "INK": "AeInk", "DSC": "AeSunDisc",
         "GNG": "AeGradeNightGain", "GNL": "AeGradeNightLift", "GAG": "AeGradeDawnGain", "GAL": "AeGradeDawnLift",
         "GDG": "AeGradeDayGain", "GDL": "AeGradeDayLift", "GKG": "AeGradeDuskGain", "GKL": "AeGradeDuskLift"}
cel = asset("M_LookCel", unreal.Material, unreal.MaterialFactoryNew())
look_source.custom_material(cel, cel_code, {"WP": unreal.MaterialExpressionWorldPosition, "WPU": unreal.MaterialExpressionWorldPosition, "NW": unreal.MaterialExpressionVertexNormalWS,
                                            "VC": unreal.MaterialExpressionVertexColor, "UVA": unreal.MaterialExpressionTextureCoordinate, "UVB": unreal.MaterialExpressionTextureCoordinate,
                                            "SH": shadow_texture}, named, collection, EXTRA)

cel.set_editor_property("used_with_skeletal_mesh", True)   # a thing with a skeleton may wear it
mel.recompile_material(cel)

# ---- the sky: the generated one, then the sun the key light is (a pale disc with a hard edge and a
# thin halo; not drawn for the moon) and stars by the style's own star count
SKY = """
float3 kd = normalize(S1.xyz);
if (S1.w < 0.5 && LK.x > 0.5) {
  float sm = dot(d, kd);
  float3 pale = lerp(sky, float3(1.5, 1.38, 1.3), DSC.z);
  sky = lerp(sky, sky + float3(0.5, 0.36, 0.3), DSC.w * saturate(1.0 - acos(clamp(sm, -1.0, 1.0)) / (DSC.x * 3.0)));
  sky = lerp(sky, pale, ae.soft(cos(DSC.x), DSC.y, sm));
}
if (S14.w > 0.0 && up > 0.0) {
  float3 q = d * 190.0; float3 cell = floor(q); float3 fr = q - cell - 0.5;
  float h = frac(sin(dot(cell, float3(12.9898, 78.233, 37.719))) * 43758.5453);
  sky += float3(0.95, 1.0, 1.3) * S14.w * step(0.9965, h) * step(length(fr), 0.2 + 0.25 * frac(h * 91.7)) * saturate(up * 5.0);
}
return ae.post(sky, S18, gr);
"""
sky_code = must_replace(look_source.AE_SKY, "return ae.post(sky, S18, gr);\n", SKY)
sky = asset("M_LookSky", unreal.Material, unreal.MaterialFactoryNew())
look_source.custom_material(sky, sky_code, {"WP": unreal.MaterialExpressionWorldPosition}, named, collection, EXTRA)

# ---- the post pass, before bloom. Depth is all it reads beside the picture, so it needs no
# buffer the forward tier lacks.
#   Inner lines: 1/depth is a straight line across any flat face, so its second difference is
#   nought on a face and jumps at a crease. Silhouettes are left to the generated ink.
#   Fog: exponential height fog, worked along the ray, in the horizon's own colour toward where
#   the pixel looks, so far ground meets the sky without a seam.
#   Grade: the four times of day weighted by the clock.
POST = look_source.AE_HELPERS + """
float d = D0.r;
float3 c = C.rgb;
if (LK.x < 0.5) return c;
bool gr = FX.z < -0.5;
bool isfar = d > FG2.z * 100.0;
float ic = 1.0 / max(d, 1.0);
float e = max(abs(1.0 / max(D1.r, 1.0) + 1.0 / max(D2.r, 1.0) - 2.0 * ic), abs(1.0 / max(D3.r, 1.0) + 1.0 / max(D4.r, 1.0) - 2.0 * ic)) / ic;
float gap = max(max(abs(D1.r - d), abs(D2.r - d)), max(abs(D3.r - d), abs(D4.r - d))) / max(d, 1.0);
float crease = saturate((e - INK.x) / max(INK.y, 0.000001)) * saturate(S31.x) * INK.z * saturate(1.0 - d * 0.01 / max(INK.w, 1.0));
if (!isfar && gap < 0.03) c = lerp(c, c * max(S6.w, 0.04), crease);
float3 ray = WP.xyz - CP.xyz; float dist = length(ray); float3 rd = ray / max(dist, 1.0);
if (!isfar) {
  float dm = max(dist * 0.01 - FG2.x, 0.0);
  float hk = max(FOG.y, 0.01);
  float eh = clamp((CP.z * 0.01 - FOG.z) / hk, -6.0, 30.0);
  float slope = rd.z * dm / hk;
  float along = abs(slope) > 0.001 ? (1.0 - exp(-clamp(slope, -30.0, 30.0))) / slope : 1.0;
  float fog = min(1.0 - exp(-FOG.x * dm * exp(-eh) * along), FOG.w);
  if (FG2.y > 0.5) fog = floor(fog / max(FOG.w, 0.001) * FG2.y + 0.5) / FG2.y * FOG.w;
  float3 sun = normalize(S1.xyz); if (S1.w > 0.5) sun = -sun;
  float cs = (rd.x * sun.x + rd.y * sun.z) / (sqrt(max(rd.x * rd.x + rd.y * rd.y, 0.000001)) * sqrt(max(sun.x * sun.x + sun.z * sun.z, 0.000001)));
  float side = saturate(cs * 0.5 + 0.5);
  float3 hz = lerp(S13.xyz, S11.xyz, (0.33 + 0.67 * side) * S11.w);
  c = lerp(c, ae.post(hz, S18, gr), fog);
}
float clock = LK.y;
float wn = max(0.0, 1.0 - min(clock, 1.0 - clock) / 0.17);
float wa = max(0.0, 1.0 - abs(clock - 0.27) / 0.14);
float wd = max(0.0, 1.0 - abs(clock - 0.5) / 0.16);
float wk = max(0.0, 1.0 - abs(clock - 0.77) / 0.14);
float ws = max(wn + wa + wd + wk, 0.0001);
float4 gain = (wn * GNG + wa * GAG + wd * GDG + wk * GKG) / ws;
float4 lift = (wn * GNL + wa * GAL + wd * GDL + wk * GKL) / ws;
if (!gr) {
  float l = ae.luma(c);
  c = max(l + (c - l) * gain.w, 0.0) * gain.xyz;
  float3 t = saturate(c);
  c += (t * t * (3.0 - 2.0 * t) - t) * lift.w;
  c += lift.xyz * (1.0 - saturate(ae.luma(c) * 2.5));
}
float2 v = float2((UV.x * 2.0 - 1.0), UV.y * 2.0 - 1.0);
c *= 1.0 - S10.w * ae.soft(1.1, 0.6, dot(v, v));
return c;
"""
post = asset("M_LookPost", unreal.Material, unreal.MaterialFactoryNew())
post.set_editor_property("material_domain", unreal.MaterialDomain.MD_POST_PROCESS)
post.set_editor_property("blendable_location", unreal.BlendableLocation.BL_SCENE_COLOR_BEFORE_BLOOM)
post.set_editor_property("blendable_priority", 10)


def scene_texture(which, dx=0, dy=0):
    def build(mat, x, y):
        node = mel.create_material_expression(mat, unreal.MaterialExpressionSceneTexture, x, y)
        node.set_editor_property("scene_texture_id", which)
        if dx or dy:
            screen = mel.create_material_expression(mat, unreal.MaterialExpressionScreenPosition, x - 800, y)
            texel = mel.create_material_expression(mat, unreal.MaterialExpressionSceneTexelSize, x - 800, y + 40)
            step = mel.create_material_expression(mat, unreal.MaterialExpressionConstant2Vector, x - 800, y + 80)
            step.set_editor_property("r", float(dx))
            step.set_editor_property("g", float(dy))
            scaled = mel.create_material_expression(mat, unreal.MaterialExpressionMultiply, x - 550, y)
            mel.connect_material_expressions(texel, "", scaled, "A")
            mel.connect_material_expressions(step, "", scaled, "B")
            moved = mel.create_material_expression(mat, unreal.MaterialExpressionAdd, x - 300, y)
            mel.connect_material_expressions(screen, "ViewportUV", moved, "A")
            mel.connect_material_expressions(scaled, "", moved, "B")
            mel.connect_material_expressions(moved, "", node, "UVs")
        return node, "Color"
    return build


def viewport_uv(mat, x, y):
    return mel.create_material_expression(mat, unreal.MaterialExpressionScreenPosition, x, y), "ViewportUV"


depth = unreal.SceneTextureId.PPI_SCENE_DEPTH
look_source.custom_material(post, POST, {"D0": scene_texture(depth), "D1": scene_texture(depth, 1, 0), "D2": scene_texture(depth, -1, 0), "D3": scene_texture(depth, 0, 1), "D4": scene_texture(depth, 0, -1),
                                         "C": scene_texture(unreal.SceneTextureId.PPI_POST_PROCESS_INPUT0), "WP": unreal.MaterialExpressionWorldPosition,
                                         "CP": unreal.MaterialExpressionCameraPositionWS, "UV": viewport_uv}, named, collection, EXTRA, unlit=False)
assets.save_directory(LOOK, only_if_is_dirty=False, recursive=True)

# ---- the level: the volume, the sun and the lights
level_path = LOOK + "/L_Look"
if assets.does_asset_exist(level_path):
    levels.load_level(level_path)
    for a in actors.get_all_level_actors():
        if isinstance(a, (unreal.PostProcessVolume, unreal.DirectionalLight, unreal.PointLight)):
            actors.destroy_actor(a)
else:
    if not levels.new_level(level_path):
        raise SystemExit("look.py: could not make the level " + level_path)

volume = actors.spawn_actor_from_class(unreal.PostProcessVolume, unreal.Vector(0, 0, 0))
volume.set_actor_label("LookPost")
volume.set_editor_property("unbound", True)
volume.set_editor_property("priority", 1.0)
s = volume.get_editor_property("settings")
for key, value in (("bloom_method", unreal.BloomMethod.BM_SOG), ("bloom_intensity", 0.5), ("bloom_threshold", 1.0),
                   ("tone_curve_amount", 0.0), ("expand_gamut", 0.0), ("blue_correction", 0.0),
                   ("vignette_intensity", 0.0), ("film_grain_intensity", 0.0), ("lens_flare_intensity", 0.0), ("bloom_dirt_mask_intensity", 0.0),
                   ("scene_fringe_intensity", 0.0), ("motion_blur_amount", 0.0), ("ambient_occlusion_intensity", 0.0)):
    s.set_editor_property("override_" + key, True)
    s.set_editor_property(key, value)
s.set_editor_property("weighted_blendables", unreal.WeightedBlendables(array=[unreal.WeightedBlendable(weight=1.0, object=post)]))
volume.set_editor_property("settings", s)

# A real sun and real lights. The cel world is unlit (it writes its own two tones), so these
# light only what a project brings with a lit material: a replacement mesh, a prop.
sun = actors.spawn_actor_from_class(unreal.DirectionalLight, unreal.Vector(0, 0, 1000))
sun.set_actor_label("LookSun")
sun.set_editor_property("tags", ["AethyrSun"])
sun.root_component.set_mobility(unreal.ComponentMobility.MOVABLE)
sun.light_component.set_editor_property("intensity", 3.0)
sun.light_component.set_editor_property("cast_shadows", True)
for k in range(manifest["uniforms"]["light_count"]):
    lamp = actors.spawn_actor_from_class(unreal.PointLight, unreal.Vector(0, 0, -100000))
    lamp.set_actor_label("LookLight%d" % k)
    lamp.set_editor_property("tags", ["AethyrLight%d" % k])
    lamp.root_component.set_mobility(unreal.ComponentMobility.MOVABLE)
    lamp.light_component.set_editor_property("intensity", 3000.0)
    lamp.light_component.set_editor_property("cast_shadows", False)
if not levels.save_current_level():
    raise SystemExit("look.py: could not save " + level_path)
levels.load_level("/Engine/Maps/Entry")

# ---- the rows
table = assets.load_asset(TABLE)
if table is None:
    raise SystemExit("look.py: no %s (run the importer first)" % TABLE)
rows = {}
for row, value in zip(lib.get_data_table_row_names(table), lib.get_data_table_column_as_string(table, "Asset")):
    rows[str(row)] = str(value)
for row, name in (("look.level", "L_Look"), ("look.params", "MPC_Look"), ("look.shadow", "RT_LookShadow"), ("material.cel", "M_LookCel"), ("material.sky", "M_LookSky")):
    rows[row] = "%s/%s.%s" % (LOOK, name, name)
if not lib.fill_data_table_from_json_string(table, json.dumps([{"Name": k, "Asset": v} for k, v in sorted(rows.items())])):
    raise SystemExit("look.py: the overrides table refused the rows")
assets.save_asset(TABLE, only_if_is_dirty=False)
unreal.AethyrEditorChecks.finish_compiling()
errors = {m.get_name(): [str(e) for e in unreal.AethyrEditorChecks.material_errors(m)] for m in (cel, sky, post)}
if any(errors.values()):
    raise SystemExit("look.py: a material did not compile: " + json.dumps(errors))
unreal.log("AETHYR_LOOK made %s, %d rows, shadow %dx%d" % (LOOK, len(rows), shadow.get_editor_property("size_x"), shadow.get_editor_property("size_y")))
