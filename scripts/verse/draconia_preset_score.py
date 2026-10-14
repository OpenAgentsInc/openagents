#!/usr/bin/env python3
"""Rank MetaHuman Creator presets by how close their faces come to the
reference (round 7).

    draconia_preset_score.py RENDER_DIR BUILD_DIR

RENDER_DIR holds each preset's 0:54-framed render and the tracker's
landmarks (landmarks/NAME.json); BUILD_DIR/NAME/head.txt is its rest head
in DNA centimetres (x, y up, z forward). Two kinds of error, each as a
share of the reference's value:

- the tracker's proportions (draconia_landmarks.py) against the 0:54
  still, smiling as the render is;
- the face's shape in 3D against the 0:24 still (near frontal), hand-marked
  there because the tracker gives no jaw: cheek width at the nose's base,
  jaw width at the mouth, eye line to chin, all over the eyes' spacing.

The score is the mean absolute error, the shape terms counted twice (the
coordinator's read: the face's breadth is the biggest miss). Prints the
ranking and writes RENDER_DIR/scores.json.
"""

import json
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(__file__))
import draconia_landmarks  # noqa: E402

REF = os.path.expanduser("~/work/film-out/draconia/parity/landmarks/owner-target-54s.json")
# Hand-marked on the 0:24 still (pixels): eye centres 1015,412 and 1195,407;
# cheek edges at the nose's base 960 and 1295; jaw edges at the mouth 985
# and 1280; chin 1115,695.
REF_IOD = 180.0
REF_SHAPE = {"cheek width": 335 / REF_IOD, "jaw width at mouth": 295 / REF_IOD, "eye line to chin": 285 / REF_IOD}
EYE_L = np.array([3.02, 149.28, 8.54])  # the archetype's eye centres (cm)
EYE_R = np.array([-3.04, 149.27, 8.52])


def shape(head_txt):
    v = np.loadtxt(head_txt)
    iod = np.linalg.norm(EYE_L - EYE_R)
    ey, ez = (EYE_L[1] + EYE_R[1]) / 2, (EYE_L[2] + EYE_R[2]) / 2
    front = v[v[:, 2] > ez - 6.0]

    def width_at(y):
        band = front[np.abs(front[:, 1] - y) < 0.4]
        return band[:, 0].max() - band[:, 0].min()

    mid = v[(np.abs(v[:, 0]) < 0.6) & (v[:, 2] > ez - 3.0)]
    # The chin's bottom: the lowest front-midline point before the neck
    # turns back (the surface still facing forward).
    chin_y = mid[mid[:, 2] > mid[:, 2].max() - 3.5][:, 1].min()
    nose_base = ey - 4.3
    mouth = ey - 7.0
    return {"cheek width": width_at(nose_base) / iod, "jaw width at mouth": width_at(mouth) / iod, "eye line to chin": (ey - chin_y) / iod}


def main():
    rdir, bdir = sys.argv[1], sys.argv[2]
    ref, _ = draconia_landmarks.features(draconia_landmarks.load(REF))
    rows = []
    for fn in sorted(os.listdir(os.path.join(rdir, "landmarks"))):
        if not fn.endswith(".json"):
            continue
        name = fn[:-5]
        try:
            f, _ = draconia_landmarks.features(draconia_landmarks.load(os.path.join(rdir, "landmarks", fn)))
        except Exception as e:  # a face the tracker lost
            print(f"{name}: no landmarks ({e})")
            continue
        errs = {k: (f[k] - ref[k]) / ref[k] for k in ref if k in f and ref[k]}
        head = os.path.join(bdir, name, "head.txt")
        if os.path.exists(head):
            s = shape(head)
            errs.update({k: (s[k] - REF_SHAPE[k]) / REF_SHAPE[k] for k in s})
        w = [2.0 if k in REF_SHAPE else 1.0 for k in errs]
        score = float(np.average([abs(e) for e in errs.values()], weights=w))
        rows.append((score, name, errs))
    rows.sort()
    for score, name, errs in rows:
        worst = sorted(errs.items(), key=lambda kv: -abs(kv[1]))[:3]
        print(f"{name:10s} {score * 100:5.1f}%  " + ", ".join(f"{k} {v * 100:+.0f}%" for k, v in worst))
    json.dump([{"name": n, "score": s, "errors": e} for s, n, e in rows], open(os.path.join(rdir, "scores.json"), "w"), indent=1)


if __name__ == "__main__":
    main()
