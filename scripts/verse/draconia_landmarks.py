#!/usr/bin/env python3
"""Draconia's face proportions against the photoreal reference.

    draconia_landmarks.py compare REF.json OURS.json [--extra EXTRA.json] [--overlay REF.png OURS.png OUT.png]

REF.json and OURS.json are MetaHuman's tracker output on a reference still
and on our render (`scripts/unreal/landmarks.py`). Every feature is a
distance divided by the distance between the eye centres (IOD), so the
two frames' scale and framing don't matter; the head's turn does a
little, which is why each reference row is compared with the render of
the same shot. The tracker gives lips, philtrum, eyelids and nasolabial
lines, not the jaw: face width and the chin come from EXTRA.json, a
hand-marked {image name: {"chin": [x, y], "cheek_l": [x, y],
"cheek_r": [x, y]}} (outer cheek edge at the nose base's height). The
nose's base is where the nasolabial lines leave the nostril wings.

Prints one line per feature: reference, ours, error (ours - reference)
as a share of the reference. Measurement only; reference stills and their
landmarks stay out of the repository.
"""

import json
import sys

import numpy as np


def load(path):
    d = json.load(open(path))
    return {k: np.array(v, float) for k, v in d["curves"].items() if len(v)}


def features(c, extra=None):
    eye_l = np.concatenate([c["crv_eyelid_upper_l"], c["crv_eyelid_lower_l"]]).mean(0)
    eye_r = np.concatenate([c["crv_eyelid_upper_r"], c["crv_eyelid_lower_r"]]).mean(0)
    iod = np.linalg.norm(eye_l - eye_r)
    eyes = (eye_l + eye_r) / 2
    up = np.array([-(eye_l - eye_r)[1], (eye_l - eye_r)[0]]) / iod  # perpendicular to the eye line
    if up[1] > 0:
        up = -up  # image y grows down; "up" points to smaller y

    def below(p):
        # Distance below the eye line, along the face's own vertical.
        return float(np.dot(p - eyes, -up))

    def width(a, b):
        return float(np.linalg.norm(a - b))

    # The nose's base: the tops of the two nasolabial lines (where they
    # leave the nostril wings); the tracker's philtrum curves end at the lip.
    tops = [c[k][np.argmin([below(p) for p in c[k]])] for k in ("crv_nasolabial_l", "crv_nasolabial_r")]
    subnasale = (tops[0] + tops[1]) / 2
    upper = np.concatenate([c["crv_lip_upper_outer_l"], c["crv_lip_upper_outer_r"]])
    lower = np.concatenate([c["crv_lip_lower_outer_l"], c["crv_lip_lower_outer_r"]])
    upper_in = np.concatenate([c["crv_lip_upper_inner_l"], c["crv_lip_upper_inner_r"]])
    lower_in = np.concatenate([c["crv_lip_lower_inner_l"], c["crv_lip_lower_inner_r"]])
    lips = np.concatenate([upper, lower])
    ax = (eye_l - eye_r) / iod
    order = np.argsort(lips @ ax)
    corners = lips[[order[0], order[-1]]]
    top_lip = upper[np.argmin([below(p) for p in upper])]
    bottom_lip = lower[np.argmax([below(p) for p in lower])]

    def eye_width(side):
        t = np.concatenate([c[f"crv_eyelid_upper_{side}"], c[f"crv_eyelid_lower_{side}"]]) @ ax
        return float(t.max() - t.min())

    def eye_open(side):
        return float(np.median([below(p) for p in c[f"crv_eyelid_lower_{side}"]]) - np.median([below(p) for p in c[f"crv_eyelid_upper_{side}"]]))

    f = {
        "eye width (mean of both) / IOD": (eye_width("l") + eye_width("r")) / 2 / iod,
        "eye opening / IOD": (eye_open("l") + eye_open("r")) / 2 / iod,
        "eye line to nose base (midface) / IOD": below(subnasale) / iod,
        "nose base to upper lip / IOD": (below(top_lip) - below(subnasale)) / iod,
        "upper lip height / IOD": (np.median([below(p) for p in upper_in]) - below(top_lip)) / iod,
        "lower lip height / IOD": (below(bottom_lip) - np.median([below(p) for p in lower_in])) / iod,
        "mouth width / IOD": width(*corners) / iod,
    }
    if extra:
        chin = np.array(extra["chin"], float)
        cw = width(np.array(extra["cheek_l"], float), np.array(extra["cheek_r"], float))
        f["eye line to chin (lower face) / IOD"] = below(chin) / iod
        f["lip to chin / IOD"] = (below(chin) - below(bottom_lip)) / iod
        f["face width (cheeks) / IOD"] = cw / iod
        f["face width / eye-to-chin height"] = cw / below(chin)
    for k in list(f):
        f[k] = abs(float(f[k]))
    return f, {"eye_l": eye_l, "eye_r": eye_r, "subnasale": subnasale, "top_lip": top_lip, "bottom_lip": bottom_lip, "corners": corners}


def overlay(img_path, curves, marks, out):
    from PIL import Image, ImageDraw

    im = Image.open(img_path).convert("RGB")
    d = ImageDraw.Draw(im)
    for pts in curves.values():
        d.line([tuple(p) for p in pts], fill=(255, 220, 0), width=2)
    for k, p in marks.items():
        ps = p if p.ndim == 2 else [p]
        for q in ps:
            d.ellipse((q[0] - 6, q[1] - 6, q[0] + 6, q[1] + 6), outline=(255, 0, 80), width=3)
    im.save(out)


def main():
    a = sys.argv[1:]
    if not a or a[0] != "compare":
        raise SystemExit(__doc__)
    ref, ours = a[1], a[2]
    extra = json.load(open(a[a.index("--extra") + 1])) if "--extra" in a else {}
    name = lambda p: p.rsplit("/", 1)[-1].rsplit(".", 1)[0]  # noqa: E731
    cr, co = load(ref), load(ours)
    fr, mr = features(cr, extra.get(name(ref)))
    fo, mo = features(co, extra.get(name(ours)))
    print(f"{'feature':44s} {'reference':>9s} {'ours':>8s} {'error':>8s}")
    for k in fr:
        if k in fo:
            e = (fo[k] - fr[k]) / fr[k] * 100 if fr[k] else float("nan")
            print(f"{k:44s} {fr[k]:9.3f} {fo[k]:8.3f} {e:+7.1f}%")
    if "--overlay" in a:
        i = a.index("--overlay")
        overlay(a[i + 1], cr, mr, a[i + 3].replace(".png", "-ref.png"))
        overlay(a[i + 2], co, mo, a[i + 3].replace(".png", "-ours.png"))


if __name__ == "__main__":
    main()
