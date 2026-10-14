"""Face landmarks on stills with MetaHuman's own tracker (UE 5.8, local).

Run with scripts/unreal/run_ue.sh:

    DRACONIA_LANDMARK_IMAGES="a.png b.png" DRACONIA_LANDMARK_OUT=dir \
        scripts/unreal/run_ue.sh scripts/unreal/landmarks.py

For every image, `track_face_landmarks_from_image` (the tracker MetaHuman
Creator uses to conform to a portrait) finds the face's contour curves;
they are written as dir/<image name>.json, {curve: [[x, y], ...]} in
pixels. `scripts/verse/draconia_landmarks.py` turns two of these into
per-feature proportions and their error, so Draconia's renders are
measured against the photoreal reference with the same tracker on both.
Measurement only: nothing here is used to train anything, and reference
stills stay out of the repository.
"""

import json
import os
import traceback

import unreal

IMAGES = os.environ.get("DRACONIA_LANDMARK_IMAGES", "").split()
OUT = os.environ.get("DRACONIA_LANDMARK_OUT", os.path.expanduser("~/work/film-out/draconia/parity/landmarks"))


def log(msg):
    print(f"DRACONIA {msg}", flush=True)
    unreal.log(f"DRACONIA {msg}")


SHOWN = []


def points(tp):
    if not SHOWN:
        SHOWN.append(1)
        log(f"TrackingPoints fields: {[n for n in dir(tp) if not n.startswith('_')]}")
    for name in dir(tp):
        if name.startswith("_"):
            continue
        try:
            v = getattr(tp, name)
        except Exception:
            continue
        if isinstance(v, unreal.Array) and len(v) and hasattr(v[0], "x"):
            return [[p.x, p.y] for p in v]
    return []


def main():
    os.makedirs(OUT, exist_ok=True)
    sub = unreal.get_editor_subsystem(unreal.MetaHumanCharacterEditorSubsystem)
    for path in IMAGES:
        try:
            size, pixels = unreal.PromotedFrameUtils.get_promoted_frame_as_pixel_array_from_disk(path)
            result = sub.track_face_landmarks_from_image(pixels, size.x, size.y)
            if isinstance(result, tuple) and len(result) == 1:
                result = result[0]
            if not result:
                log(f"no face in {path}")
                continue
            curves = {str(k): points(v) for k, v in result.items()}
            name = os.path.splitext(os.path.basename(path))[0]
            with open(os.path.join(OUT, name + ".json"), "w") as f:
                json.dump({"size": [size.x, size.y], "curves": curves}, f)
            log(f"{name}: {len(curves)} curves")
        except Exception:
            log(f"{path} failed: " + traceback.format_exc().replace("\n", " | "))
    log("done")
    unreal.SystemLibrary.quit_editor()


main()
