# Offline convex decomposition with CoACD (https://github.com/SarahWeiii/CoACD,
# MIT licence; Wei et al., "Approximate Convex Decomposition for 3D Meshes
# with Collision-Aware Concavity and Tree Search", SIGGRAPH 2022).
#
# Run by crates/cad/src/collision.rs as `python3 -c <this file>`; never
# linked into the engine. Reads one JSON object on stdin:
#   {"vertices": [[x, y, z], ...], "triangles": [[a, b, c], ...], "params": {...}}
# and writes {"tool": "coacd", "version": ..., "params": ..., "parts":
# [{"vertices": [...], "triangles": [...]}, ...]} on stdout.
import json
import sys

import numpy as np

import coacd

try:
    from importlib.metadata import version

    VERSION = version("coacd")
except Exception:  # noqa: BLE001 - a missing version is recorded as unknown
    VERSION = "unknown"

request = json.load(sys.stdin)
params = request["params"]
coacd.set_log_level("error")
mesh = coacd.Mesh(
    np.asarray(request["vertices"], dtype=np.float64),
    np.asarray(request["triangles"], dtype=np.int32),
)
parts = coacd.run_coacd(
    mesh,
    threshold=float(params["threshold"]),
    max_convex_hull=int(params["max_convex_hull"]),
    preprocess_mode=str(params["preprocess_mode"]),
    preprocess_resolution=int(params["preprocess_resolution"]),
    resolution=int(params["resolution"]),
    mcts_nodes=int(params["mcts_nodes"]),
    mcts_iterations=int(params["mcts_iterations"]),
    mcts_max_depth=int(params["mcts_max_depth"]),
    merge=bool(params["merge"]),
    max_ch_vertex=int(params["max_ch_vertex"]),
    seed=int(params["seed"]),
)
json.dump(
    {
        "tool": "coacd",
        "version": VERSION,
        "params": params,
        "parts": [
            {"vertices": np.asarray(v).tolist(), "triangles": np.asarray(t).tolist()}
            for v, t in parts
        ],
    },
    sys.stdout,
)
