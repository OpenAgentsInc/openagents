#!/usr/bin/env python3
"""Blend MetaHuman Creator presets' heads, vertex for vertex (round 7).

    draconia_preset_blend.py OUT.txt NAME:WEIGHT [NAME:WEIGHT ...]

Each NAME's rest head (~/work/film-out/draconia/metahuman/build-presets/
NAME/head.txt, DNA centimetres, MetaHuman's face topology, aligned to the
archetype by draconia_presets.sh) is averaged with its weight, as Creator
blends presets in its identity space; the result goes to
`metahuman_import.py` (MH_SHAPE=blend MH_BLEND_HEAD=OUT.txt), whose small
edits and Creator's own fit (draconia_rebuild.py) follow. Private
MetaHuman data, never used to train AI models.
"""

import os
import sys

import numpy as np

BUILD = os.path.expanduser("~/work/film-out/draconia/metahuman/build-presets")


def main():
    out, pairs = sys.argv[1], sys.argv[2:]
    total, acc = 0.0, None
    for p in pairs:
        name, w = p.rsplit(":", 1)
        w = float(w)
        v = np.loadtxt(os.path.join(BUILD, name, "head.txt"))
        acc = v * w if acc is None else acc + v * w
        total += w
    np.savetxt(out, acc / total, fmt="%.5f")
    print("blended", pairs, "->", out)


if __name__ == "__main__":
    main()
