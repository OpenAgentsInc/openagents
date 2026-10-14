"""Face outline measures for Draconia parity (round 18).

MetaHuman's tracker (`draconia_landmarks.py`) measures the eyes, nose and
lips but not the face's outline, and in round 17 every inner feature was
within 7% of the reference while the face still read broad, round-cheeked
and soft-jawed. This measures the outline: MediaPipe's face landmarker
(Apache-2.0, run from a local virtualenv, not a repo dependency) finds the
face oval in each image, and the oval's width is read at levels between
the eye line (0) and the chin (1), over the distance between the pupils.
Also the chin's taper (the angle between the outline's two sides over the
lowest fifth) and the face's length.

Usage (with mediapipe installed):
    python draconia_outline.py MODEL.task ref.png ours.png [more.png ...]
prints one column per image; the first is the reference the others are
compared against.

Renders measured here are of our own character; no image is kept for any
training (see docs/verse/private-assets.md).
"""

import math
import sys

import numpy as np

LEVELS = (0.0, 0.2, 0.4, 0.6, 0.75, 0.9)


def landmarks(model, path):
    import mediapipe as mp
    from mediapipe.tasks import python as mpt
    from mediapipe.tasks.python import vision

    opts = vision.FaceLandmarkerOptions(base_options=mpt.BaseOptions(model_asset_path=model), num_faces=1)
    det = vision.FaceLandmarker.create_from_options(opts)
    img = mp.Image.create_from_file(path)
    res = det.detect(img)
    if not res.face_landmarks:
        return None
    w, h = img.width, img.height
    return np.array([[p.x * w, p.y * h] for p in res.face_landmarks[0]])


def oval(lm):
    from mediapipe.tasks.python.vision import FaceLandmarksConnections as C

    edges = [(c.start, c.end) for c in C.FACE_LANDMARKS_FACE_OVAL]
    nxt = {a: b for a, b in edges}
    start = edges[0][0]
    order = [start]
    while nxt[order[-1]] != start and len(order) < 100:
        order.append(nxt[order[-1]])
    return lm[order]


def width_at(poly, y):
    xs = []
    n = len(poly)
    for i in range(n):
        (x0, y0), (x1, y1) = poly[i], poly[(i + 1) % n]
        if (y0 - y) * (y1 - y) <= 0 and y0 != y1:
            xs.append(x0 + (y - y0) / (y1 - y0) * (x1 - x0))
    return (max(xs) - min(xs)) if len(xs) >= 2 else float("nan")


def measure(lm):
    # The eyes' centres from their corners (the irises move with the gaze).
    pl, pr = (lm[33] + lm[133]) / 2, (lm[362] + lm[263]) / 2
    iod = np.linalg.norm(pl - pr)
    # Rotate so the eyes are level.
    a = math.atan2(pr[1] - pl[1], pr[0] - pl[0])
    c, s = math.cos(-a), math.sin(-a)
    R = np.array([[c, -s], [s, c]])
    p = (lm - pl) @ R.T
    eye_y = (p[33, 1] + p[133, 1] + p[362, 1] + p[263, 1]) / 4
    chin_y = p[152, 1]
    poly = oval(p)
    out = {}
    for t in LEVELS:
        out[f"width at {t:.2f} (eyes 0, chin 1)"] = width_at(poly, eye_y + t * (chin_y - eye_y)) / iod
    # The chin's taper: the outline's direction on each side between 80%
    # and 95% of the way down, the angle between them.
    y0, y1 = eye_y + 0.8 * (chin_y - eye_y), eye_y + 0.95 * (chin_y - eye_y)
    w0, w1 = width_at(poly, y0), width_at(poly, y1)
    out["chin taper angle (deg)"] = 2 * math.degrees(math.atan2((w0 - w1) / 2, y1 - y0))
    out["eye line to chin / IOD"] = (chin_y - eye_y) / iod
    out["eye line to nose base / IOD"] = (p[2, 1] - eye_y) / iod
    out["nose base to mouth / IOD"] = ((p[13, 1] + p[14, 1]) / 2 - p[2, 1]) / iod
    out["mouth to chin / IOD"] = (chin_y - (p[13, 1] + p[14, 1]) / 2) / iod
    out["brow to eye line / IOD"] = (eye_y - (p[105, 1] + p[334, 1]) / 2) / iod
    out["cheek (0.40) / jaw (0.75) width"] = out["width at 0.40 (eyes 0, chin 1)"] / out["width at 0.75 (eyes 0, chin 1)"]
    # Eye size: corner to corner over IOD, and opening.
    out["eye width / IOD"] = (np.linalg.norm(p[33] - p[133]) + np.linalg.norm(p[362] - p[263])) / 2 / iod
    out["eye opening / IOD"] = (np.linalg.norm(p[159] - p[145]) + np.linalg.norm(p[386] - p[374])) / 2 / iod
    out["nose width / IOD"] = np.linalg.norm(p[129] - p[358]) / iod
    out["mouth width / IOD"] = np.linalg.norm(p[61] - p[291]) / iod
    return out


def main():
    model, *paths = sys.argv[1:]
    cols = []
    for path in paths:
        lm = landmarks(model, path)
        cols.append(measure(lm) if lm is not None else None)
    keys = list(next(c for c in cols if c).keys())
    print(f"{'measure':40s}" + "".join(f"{p.split('/')[-1][:14]:>16s}" for p in paths))
    for k in keys:
        row = f"{k:40s}"
        for i, c in enumerate(cols):
            if c is None:
                row += f"{'-':>16s}"
                continue
            v = c[k]
            if i == 0 or cols[0] is None:
                row += f"{v:16.3f}"
            else:
                row += f"{v:9.3f} {100 * (v / cols[0][k] - 1):+5.1f}%"
        print(row)


if __name__ == "__main__":
    main()
