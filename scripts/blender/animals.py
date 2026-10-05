"""Build stylized low-poly animals on simple rigs and write them as glTF.

Run headless:
    Blender -b --factory-startup --python scripts/blender/animals.py -- [OUT_DIR] [NAME ...]

Writes `bear.glb`, `wolf.glb`, and `eagle.glb` for Wild Shape, until real
packs exist. Each body is built from low-poly solids, every solid weighted
fully to one bone (rigid skinning), and its clips are keyed here:

- bear and wolf: `idle` and `walk` (a four-beat walk; the wolf's is a trot).
- eagle: `idle` (perched, wings folded) and `flap` (wings spread, beating).

Every bone points up +Z from its pivot, so a bone's local X is world X:
`rx` pitches (legs swing, heads nod), `ry` yaws, and `rz` rolls about the
forward axis. Animals face -Y in Blender, which is +Z (glTF's front) after
export; their feet stand at the origin's height.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

TAU = 2 * math.pi


def part(obj, bone):
    return kit.bind(obj, bone)


def up(p, h=0.1):
    return (p[0], p[1], p[2] + h)


def rig_and_skin(name, bones, parts):
    body = kit.join(name, parts)
    kit.flat([body])
    arm = kit.armature(name + "_Rig", [(b, parent, head, up(head)) for b, parent, head in bones])
    kit.skin(arm, body)
    return arm, body


# --- Quadrupeds ---------------------------------------------------------------


def quadruped(name, p):
    """Build a four-legged animal from the proportions in `p`."""
    fur = kit.mat(name + "_Fur", p["fur"], 0.95)
    pale = kit.mat(name + "_Pale", p["pale"], 0.95)
    dark = kit.mat(name + "_Dark", (0.05, 0.04, 0.04), 0.5)
    L, W, Hb, hc = p["len"], p["wid"], p["ht"], p["hc"]  # body length, width, height; body center height
    parts = []

    # Body: a stretched ball, with a shoulder hump and a pale belly.
    parts.append(part(kit.ball("Body", 0.5, (0, 0, hc), fur, segs=10, rings=7, scale=(W, L, Hb)), "body"))
    parts.append(part(kit.ball("Chest", 0.5, (0, -L * 0.28, hc + Hb * 0.06), fur, segs=8, rings=6,
                               scale=(W * 1.05, L * 0.5, Hb * 1.08)), "body"))
    parts.append(part(kit.ball("Belly", 0.5, (0, 0.02, hc - Hb * 0.18), pale, segs=8, rings=5,
                               scale=(W * 0.8, L * 0.75, Hb * 0.6)), "body"))
    if p.get("hump"):
        parts.append(part(kit.ball("Hump", 0.5, (0, -L * 0.25, hc + Hb * 0.32), fur, segs=8, rings=5,
                                   scale=(W * 0.7, L * 0.4, Hb * 0.45)), "body"))

    # Legs: an upper leg from the hip or shoulder to the knee, a lower leg
    # and paw below it.
    bones = [("body", None, (0, 0, hc))]
    lr = p["leg_r"]
    for tag, y in (("F", -L * 0.3), ("B", L * 0.3)):
        for side, sx in (("L", 1), ("R", -1)):
            x = sx * W * 0.3
            hip = hc - Hb * 0.05
            knee = p["knee"]
            leg, shin = "leg_%s%s" % (tag, side), "shin_%s%s" % (tag, side)
            bones += [(leg, "body", (x, y, hip)), (shin, leg, (x, y, knee))]
            parts.append(part(kit.cyl(leg + "_m", lr * 1.25, hip - knee + lr, (x, y, (hip + knee) / 2), fur,
                                      verts=7, r2=lr * 0.95), leg))
            parts.append(part(kit.cyl(shin + "_m", lr, knee, (x, y, knee / 2 + lr * 0.3), fur, verts=7,
                                      r2=lr * 0.9), shin))
            parts.append(part(kit.ball(shin + "_paw", lr * 1.25, (x, y - lr * 0.4, lr * 0.55), fur, segs=7, rings=4, scale=(1, 1.4, 0.55)), shin))
            for k in (-1, 0, 1):
                parts.append(part(kit.ball(shin + "_claw%d" % k, lr * 0.22,
                                           (x + k * lr * 0.5, y - lr * 1.95, lr * 0.25), pale if name == "Wolf" else dark,
                                           segs=4, rings=3, scale=(1, 1.6, 1)), shin))

    # Head: on a neck at the front of the body.
    hs = p["head"]
    neck = (0, -L * 0.42, hc + Hb * 0.2)
    bones.append(("head", "body", neck))
    hx, hy, hz = 0, neck[1] - hs * 0.75, neck[2] + hs * 0.35
    parts.append(part(kit.cyl("Neck", hs * 0.62, hs * 0.9, (0, neck[1] - hs * 0.2, neck[2] + hs * 0.05), fur, verts=8,
                              rot=(math.radians(60), 0, 0)), "head"))
    parts.append(part(kit.ball("Head", hs, (hx, hy, hz), fur, segs=9, rings=6, scale=(0.95, 1.1, 0.9)), "head"))
    sl = p["snout"]
    parts.append(part(kit.cyl("Snout", hs * 0.5, sl, (hx, hy - hs * 0.8 - sl / 2 + hs * 0.25, hz - hs * 0.25), pale,
                              verts=8, r2=hs * p["snout_tip"], rot=(math.pi / 2, 0, 0)), "head"))
    tip = (hx, hy - hs * 0.8 - sl + hs * 0.2, hz - hs * 0.2)
    parts.append(part(kit.ball("Nose", hs * 0.2, tip, dark, segs=6, rings=4, scale=(1.3, 0.8, 0.9)), "head"))
    for sx in (-1, 1):
        parts.append(part(kit.ball("Eye%d" % sx, hs * 0.13, (sx * hs * 0.45, hy - hs * 0.82, hz + hs * 0.2), dark,
                                   segs=6, rings=4), "head"))
        if p["ears"] == "round":
            parts.append(part(kit.ball("Ear%d" % sx, hs * 0.3, (sx * hs * 0.62, hy + hs * 0.15, hz + hs * 0.78), fur,
                                       segs=7, rings=4, scale=(1, 0.55, 1)), "head"))
        else:
            parts.append(part(kit.cyl("Ear%d" % sx, hs * 0.3, hs * 0.7, (sx * hs * 0.48, hy + hs * 0.1, hz + hs * 1.0),
                                      fur, verts=4, r2=0.005, rot=(0, sx * math.radians(-12), math.radians(45))),
                              "head"))
            parts.append(part(kit.cyl("EarIn%d" % sx, hs * 0.16, hs * 0.45,
                                      (sx * hs * 0.48, hy + hs * 0.02, hz + hs * 0.95), dark, verts=4, r2=0.004,
                                      rot=(0, sx * math.radians(-12), math.radians(45))), "head"))
    if p.get("mane"):
        parts.append(part(kit.ball("Ruff", hs * 1.1, (0, neck[1] - hs * 0.1, neck[2] - hs * 0.05), pale, segs=8,
                                   rings=5, scale=(1.15, 0.8, 1.05)), "head"))

    # Tail.
    tail = (0, L * 0.48, hc + Hb * 0.2)
    bones.append(("tail", "body", tail))
    if p["tail"] == "stub":
        parts.append(part(kit.ball("Tail", hs * 0.32, (0, tail[1] + 0.03, tail[2]), fur, segs=6, rings=4), "tail"))
    else:
        tl = p["tail_len"]
        droop = math.radians(p["tail_droop"])
        mid = (0, tail[1] + math.cos(droop) * tl / 2, tail[2] - math.sin(droop) * tl / 2)
        # A cone pointing back: rx of -90 degrees aims a cylinder's +Z at +Y.
        parts.append(part(kit.cyl("Tail", hs * 0.32, tl, mid, fur, verts=7, r2=hs * 0.12,
                                  rot=(-math.pi / 2 - droop, 0, 0)), "tail"))
        tip_p = (0, tail[1] + math.cos(droop) * tl, tail[2] - math.sin(droop) * tl)
        parts.append(part(kit.ball("TailTip", hs * 0.17, tip_p, pale, segs=6, rings=4, scale=(1, 1.6, 1)), "tail"))

    return rig_and_skin(name, bones, parts)


def quadruped_actions(arm, gait, swing, lift, bob, sway):
    """Key `idle` and `walk` on a quadruped rig.

    `gait` holds each leg's phase offset; `swing` is the legs' pitch
    amplitude, `lift` the knee's extra bend while a foot travels forward,
    `bob` the body's vertical bounce in meters, `sway` the tail's yaw.
    """

    def idle(t):
        s = math.sin(TAU * t)
        k = {
            "body": {"rot": (0.02 * s, 0, 0), "loc": (0, 0.012 * (1 + s), 0)},
            "head": (0.06 * math.sin(TAU * t + 1.0), 0.25 * math.sin(TAU * t), 0),
            "tail": (0, sway * 0.5 * math.sin(TAU * 2 * t), 0),
        }
        for leg in gait:
            k["leg_" + leg] = (-0.02 * s, 0, 0)
            k["shin_" + leg] = (0.02 * s, 0, 0)
        return k

    def walk(t):
        k = {
            "body": {"rot": (0, 0, 0.03 * math.sin(TAU * t)), "loc": (0, bob * (1 + math.cos(TAU * 2 * t)) / 2, 0)},
            "head": (0.06 * math.cos(TAU * 2 * t), 0.06 * math.sin(TAU * t), 0),
            "tail": (0.05 * math.sin(TAU * 2 * t), sway * math.sin(TAU * t), 0),
        }
        for leg, phase in gait.items():
            a = TAU * (t + phase)
            # Positive pitch swings the foot back (+Y), so the foot travels
            # forward while sin(a) falls; that's when the joint folds the
            # lower leg back to lift the paw.
            k["leg_" + leg] = (swing * math.sin(a), 0, 0)
            k["shin_" + leg] = (lift * max(0.0, -math.cos(a)), 0, 0)
        return k

    kit.action(arm, "idle", 12, idle, step=6)
    kit.action(arm, "walk", 12, walk, step=2)


def bear(folder):
    kit.reset()
    arm, _ = quadruped(
        "Bear",
        dict(fur=(0.33, 0.2, 0.11), pale=(0.55, 0.4, 0.26), len=1.7, wid=0.8, ht=0.8, hc=0.9, knee=0.4,
             leg_r=0.11, head=0.25, snout=0.22, snout_tip=0.4, ears="round", tail="stub", hump=True),
    )
    quadruped_actions(arm, {"BL": 0.0, "FL": 0.25, "BR": 0.5, "FR": 0.75}, 0.38, 0.6, 0.03, 0.1)
    kit.export(os.path.join(folder, "bear.glb"), animations=True)


def wolf(folder):
    kit.reset()
    arm, _ = quadruped(
        "Wolf",
        dict(fur=(0.42, 0.42, 0.44), pale=(0.8, 0.78, 0.74), len=1.1, wid=0.42, ht=0.42, hc=0.68, knee=0.36,
             leg_r=0.055, head=0.16, snout=0.24, snout_tip=0.35, ears="pointed", tail="bushy", tail_len=0.55,
             tail_droop=58, mane=True),
    )
    # A trot: diagonal pairs move together.
    quadruped_actions(arm, {"FL": 0.0, "BR": 0.0, "FR": 0.5, "BL": 0.5}, 0.5, 0.8, 0.03, 0.3)
    kit.export(os.path.join(folder, "wolf.glb"), animations=True)


# --- Eagle --------------------------------------------------------------------


def wing_mesh(name, root, length, chord_in, chord_out, sx, material):
    """A flat, slightly thick wing panel from `root` outward along ±X."""
    import bmesh
    import bpy

    bm = bmesh.new()
    x0, y0, z0 = root
    t = 0.025
    outline = [(0, -chord_in * 0.35), (length, -chord_out * 0.3), (length, chord_out * 0.7), (0, chord_in * 0.65)]
    top = [bm.verts.new((x0 + sx * u, y0 + v, z0 + t)) for u, v in outline]
    bot = [bm.verts.new((x0 + sx * u, y0 + v, z0 - t)) for u, v in outline]
    bm.faces.new(top)
    bm.faces.new(list(reversed(bot)))
    for i in range(4):
        j = (i + 1) % 4
        bm.faces.new((top[i], bot[i], bot[j], top[j]))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    o = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(o)
    o.data.materials.append(material)
    return o


def eagle(folder):
    kit.reset()
    brown = kit.mat("Eagle_Brown", (0.3, 0.18, 0.09), 0.9)
    dark = kit.mat("Eagle_Feather", (0.2, 0.12, 0.06), 0.9)
    white = kit.mat("Eagle_White", (0.92, 0.9, 0.84), 0.9)
    yellow = kit.mat("Eagle_Yellow", (0.95, 0.68, 0.12), 0.6)
    black = kit.mat("Eagle_Eye", (0.04, 0.03, 0.02), 0.4)
    parts = []
    hc = 0.42  # body center height, perched
    tilt = math.radians(-35)  # the body leans back, head up, perched
    parts.append(part(kit.ball("Body", 0.5, (0, 0, hc), brown, segs=10, rings=7, scale=(0.3, 0.55, 0.32),
                               rot=(tilt, 0, 0)), "body"))
    parts.append(part(kit.ball("Breast", 0.5, (0, -0.06, hc - 0.02), brown, segs=8, rings=6,
                               scale=(0.28, 0.36, 0.34), rot=(tilt, 0, 0)), "body"))
    # Tail fan, pointing back and down.
    tail_root = (0, 0.22, hc - 0.12)
    parts.append(part(kit.box("Tail", (0.2, 0.3, 0.035), (0, 0.36, hc - 0.2), white, rot=(math.radians(-35), 0, 0)),
                      "tail"))
    # Head: white, with a hooked yellow beak.
    neck = (0, -0.12, hc + 0.14)
    head = (0, -0.18, hc + 0.3)
    parts.append(part(kit.ball("Neck", 0.12, (0, -0.15, hc + 0.2), white, segs=8, rings=5, scale=(1, 1, 1.2)), "head"))
    parts.append(part(kit.ball("Head", 0.12, head, white, segs=9, rings=6, scale=(0.95, 1.15, 0.95)), "head"))
    parts.append(part(kit.cyl("Beak", 0.045, 0.13, (0, head[1] - 0.17, head[2] - 0.02), yellow, verts=6, r2=0.012,
                              rot=(math.radians(100), 0, 0)), "head"))
    parts.append(part(kit.ball("Hook", 0.025, (0, head[1] - 0.23, head[2] - 0.05), yellow, segs=5, rings=3), "head"))
    for sx in (-1, 1):
        parts.append(part(kit.ball("Eye%d" % sx, 0.022, (sx * 0.075, head[1] - 0.08, head[2] + 0.03), black, segs=5,
                                   rings=3), "head"))
        parts.append(part(kit.box("Brow%d" % sx, (0.05, 0.05, 0.015), (sx * 0.07, head[1] - 0.08, head[2] + 0.06),
                                  white), "head"))
    # Legs and talons.
    for sx in (-1, 1):
        x = sx * 0.08
        parts.append(part(kit.ball("Thigh%d" % sx, 0.07, (x, 0.0, hc - 0.2), brown, segs=6, rings=4,
                                   scale=(1, 1, 1.4)), "body"))
        parts.append(part(kit.cyl("Shank%d" % sx, 0.022, 0.16, (x, -0.02, 0.1), yellow, verts=6), "body"))
        for k in (-1, 0, 1):
            parts.append(part(kit.cyl("Toe%d_%d" % (sx, k), 0.014, 0.09, (x + k * 0.025, -0.06, 0.015), yellow,
                                      verts=5, r2=0.006, rot=(math.radians(90), 0, math.radians(k * 25))), "body"))
        parts.append(part(kit.cyl("Back%d" % sx, 0.014, 0.07, (x, 0.03, 0.015), yellow, verts=5, r2=0.006,
                                  rot=(math.radians(-90), 0, 0)), "body"))

    # Wings: an inner panel on `wing_*`, an outer panel and finger feathers
    # on `tip_*`; the rest pose spreads them flat, 2 m tip to tip.
    bones = [("body", None, (0, 0, hc)), ("head", "body", neck), ("tail", "body", tail_root)]
    shoulder_z = hc + 0.1
    for side, sx in (("L", 1), ("R", -1)):
        sh = (sx * 0.13, -0.05, shoulder_z)
        el = (sx * 0.55, -0.05, shoulder_z)
        bones += [("wing_" + side, "body", sh), ("tip_" + side, "wing_" + side, el)]
        parts.append(part(wing_mesh("Wing" + side, sh, 0.42, 0.34, 0.36, sx, brown), "wing_" + side))
        parts.append(part(wing_mesh("Tip" + side, el, 0.28, 0.36, 0.26, sx, dark), "tip_" + side))
        for k in range(4):
            y = -0.06 + k * 0.07
            parts.append(part(kit.box("Finger%s%d" % (side, k), (0.2, 0.05, 0.02),
                                      (sx * (0.55 + 0.28 + 0.07), y, shoulder_z), dark,
                                      rot=(0, 0, sx * math.radians(-8 + k * 9))), "tip_" + side))
    arm, _ = rig_and_skin("Eagle", bones, parts)

    def fold(sx):
        # Roll the wing on edge (pitch comes first in XYZ order), then yaw
        # it back along the body; tuck the tip in a little.
        return (math.radians(-75), sx * math.radians(82), 0), (0, sx * math.radians(6), 0)

    def idle(t):
        wl, tl = fold(1)
        wr, tr = fold(-1)
        s = math.sin(TAU * t)
        look = 0.5 * math.sin(TAU * t) if (t % 0.5) < 0.4 else 0.0
        return {
            "body": {"rot": (0.02 * s, 0, 0), "loc": (0, 0.006 * (1 + s), 0)},
            "head": (0.05 * math.sin(TAU * 2 * t), look, 0),
            "wing_L": (wl[0] + 0.03 * s, wl[1], wl[2]),
            "wing_R": (wr[0] + 0.03 * s, wr[1], wr[2]),
            "tip_L": tl,
            "tip_R": tr,
            "tail": (0.05 * s, 0, 0),
        }

    def flap(t):
        a = math.sin(TAU * t)
        lag = math.sin(TAU * t - 0.9)
        return {
            # Level the body, rise off the perch, and bob with each beat.
            "body": {"rot": (math.radians(30), 0, 0), "loc": (0, 0.45 + 0.06 * -a, 0)},
            "head": (math.radians(-25), 0, 0),
            "wing_L": (0, 0, -0.75 * a + 0.1),
            "wing_R": (0, 0, 0.75 * a - 0.1),
            "tip_L": (0, 0.15 * max(0.0, -a), -0.45 * lag),
            "tip_R": (0, -0.15 * max(0.0, -a), 0.45 * lag),
            "tail": (math.radians(15) + 0.1 * a, 0, 0),
        }

    kit.action(arm, "idle", 12, idle, step=6)
    kit.action(arm, "flap", 8, flap, step=3)
    kit.export(os.path.join(folder, "eagle.glb"), animations=True)


def main():
    a = kit.args()
    folder = a[0] if a else os.path.join(kit.REPO, "assets", "verse", "generated")
    names = a[1:] or ["bear", "wolf", "eagle"]
    for n in names:
        {"bear": bear, "wolf": wolf, "eagle": eagle}[n](folder)


main()
