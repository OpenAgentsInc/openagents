# Firm reason: 6 (import.py, which this serves, runs inside the Unreal Editor's Python).
# The kit slots' plan for import.py (GitHub #13565): which slots a kit.NAME row of
# DT_AethyrOverrides replaces, where its mesh stands, and which world groups (the slot's
# blockout) the importer drops. No Unreal here, so it is checked on its own:
#   python3 -I metaverse-engine/export/unreal/kits.py [BUNDLE/manifest.json]
# The manifest's `kits` (splice-export.coax, metaverse/splice-bots/kits.coax): each slot's
# places (a point in metres and a turn, a piece's own +z being (sin yaw, cos yaw)) and the
# world groups that hold its blockout.
import json
import math
import sys

UNITS = 100.0
ROW = "kit."


def place_to_unreal(place):
    # Metaverse Engine metres, y up -> Unreal centimetres, z up (import.py to_unreal), and the turn:
    # a mesh's own axes are the slot's axes mapped the same way (its +Y is the slot's +z, into a
    # bay), so the Metaverse Engine's turn by yaw is an Unreal yaw of -yaw
    p = place["at"]
    return (p[0] * UNITS, p[2] * UNITS, p[1] * UNITS), -math.degrees(place["yaw"])


def plan(manifest, rows):
    """rows: the overrides table, row name -> asset path. Returns
    replace: [{slot, asset, places: [((x, y, z) cm, yaw degrees, open m)], drop: [group names]}],
    blockout: [{slot, why}] for every slot not replaced, and unknown: kit rows that name no slot."""
    slots = manifest.get("kits", {})
    replace, blockout = [], []
    for name in sorted(slots):
        slot = slots[name]
        places = slot.get("places", []) if isinstance(slot, dict) else []
        groups = slot.get("groups", []) if isinstance(slot, dict) else []
        asset = rows.get(ROW + name)
        if not asset:
            blockout.append({"slot": name, "why": "no %s%s row" % (ROW, name)})
        elif not places:
            blockout.append({"slot": name, "why": "the export lists no places for it yet"})
        elif not groups:
            blockout.append({"slot": name, "why": "its blockout is not marked in the world mesh"})
        else:
            replace.append({"slot": name, "asset": asset, "drop": list(groups),
                            "places": [place_to_unreal(p) + (p.get("open", 0.0),) for p in places]})
    unknown = sorted(k for k in rows if k.startswith(ROW) and k[len(ROW):] not in slots)
    return {"replace": replace, "blockout": blockout, "unknown": unknown}


# ---- the checks: python3 -I kits.py [manifest.json]
def _unreal_turn(local, yaw_degrees):
    a = math.radians(yaw_degrees)
    return (local[0] * math.cos(a) - local[1] * math.sin(a), local[0] * math.sin(a) + local[1] * math.cos(a), local[2])


def _check_manifest(manifest, want_places=None):
    bay = manifest["kits"]["garage_bay"]
    world = {g["name"]: g for g in manifest["world"]["groups"]}
    rows = {"kit.garage_bay": "/Game/Splice/Kits/SM_Bay.SM_Bay", "kit.no_such_slot": "/Game/x", "thing.raider": "/Game/y"}
    p = plan(manifest, rows)
    assert [r["slot"] for r in p["replace"]] == ["garage_bay"], p
    r = p["replace"][0]
    if want_places is not None:
        assert len(r["places"]) == want_places, len(r["places"])
    assert r["drop"] and all(world[g].get("kit") == "garage_bay" for g in r["drop"]), "a dropped group is not the bay's"
    assert sorted(r["drop"]) == sorted(n for n, g in world.items() if g.get("kit") == "garage_bay"), "a bay group is not dropped"
    assert p["unknown"] == ["kit.no_such_slot"], p["unknown"]
    left = {b["slot"] for b in p["blockout"]}
    assert left == set(manifest["kits"]) - {"garage_bay"}, left
    # a mesh's own point 1 m into the bay (Unreal local +Y) lands 1 m from the place along its turn
    for (loc, yaw, _open), src in zip(r["places"], bay["places"]):
        d = _unreal_turn((0.0, UNITS, 0.0), yaw)
        want = (src["at"][0] + math.sin(src["yaw"]), src["at"][2] + math.cos(src["yaw"]))
        assert abs(loc[0] + d[0] - want[0] * UNITS) < 0.01 and abs(loc[1] + d[1] - want[1] * UNITS) < 0.01, (loc, yaw, src)
    # no row: every slot still draws its blockout
    assert plan(manifest, {})["replace"] == [] and len(plan(manifest, {})["blockout"]) == len(manifest["kits"])
    return p


def _fixture():
    places, ring = [], 135.0
    for k in range(24):
        a = k * math.tau / 24
        places.append({"at": [math.sin(a) * ring, 0.0, -195.0 + math.cos(a) * ring], "yaw": a, "open": 3.0 if k in (0, 12) else 0.0})
    return {
        "world": {"groups": [{"name": "w_m00_x128_z125"}, {"name": "k01_m00_x128_z125", "kit": "garage_bay"}, {"name": "k01_m06_x129_z126", "kit": "garage_bay"}]},
        "kits": {"garage_bay": {"about": "a bay", "places": places, "groups": ["k01_m00_x128_z125", "k01_m06_x129_z126"]},
                 "section_sign": {"about": "a sign", "places": [], "groups": []},
                 "mountain": {"about": "the peaks", "places": [{"at": [0, 0, -900], "yaw": 0.0, "open": 0.0}], "groups": []}},
    }


if __name__ == "__main__":
    p = _check_manifest(_fixture(), 24)
    why = {b["slot"]: b["why"] for b in plan(_fixture(), {"kit.mountain": "/Game/m", "kit.section_sign": "/Game/s"})["blockout"]}
    assert why == {"garage_bay": "no kit.garage_bay row", "mountain": "its blockout is not marked in the world mesh", "section_sign": "the export lists no places for it yet"}, why
    print("kits.py: the fixture: 24 bays replaced, 2 groups dropped, the turn lands each mesh in its bay")
    if len(sys.argv) > 1:
        m = json.load(open(sys.argv[1]))
        p = _check_manifest(m, 24)
        print("kits.py: %s: %d slots, garage_bay at %d places, %d blockout groups dropped; still blockout: %s"
              % (sys.argv[1], len(m["kits"]), len(p["replace"][0]["places"]), len(p["replace"][0]["drop"]), ", ".join(b["slot"] for b in p["blockout"])))
