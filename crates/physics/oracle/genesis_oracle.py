"""Replay a physics oracle scene in Genesis and write its trace.

Reads `<dir>/<name>.scene.json` written by `cargo run -p physics --example
oracle_scenes -- <dir>` and writes `<dir>/<name>.genesis.json` in the same
trace format, for `physics::oracle` tests to compare. Offline tooling only:
nothing in Verse depends on Genesis. See README.md.

    python genesis_oracle.py <dir> tank_into_panel
"""

import json
import os
import sys
import tempfile

import numpy as np

import genesis as gs


def quat_wxyz(q):
    x, y, z, w = q
    return [w, x, y, z]


def rotate(q_wxyz, v, inverse=False):
    w, x, y, z = q_wxyz
    if inverse:
        x, y, z = -x, -y, -z
    u = np.array([x, y, z])
    v = np.asarray(v, dtype=float)
    return v + 2.0 * np.cross(u, np.cross(u, v) + w * v)


def geom(shape, material):
    friction = material["friction"]
    kind = shape["shape"]
    if kind == "cuboid":
        size = " ".join(str(h) for h in shape["half"])
        return f'<geom type="box" size="{size}" friction="{friction} 0 0"/>'
    if kind == "capsule":
        return (
            f'<geom type="capsule" size="{shape["radius"]} {shape["half_length"]}" '
            f'friction="{friction} 0 0"/>'
        )
    return f'<geom type="sphere" size="{shape["radius"]}" friction="{friction} 0 0"/>'


def mjcf(scene):
    bodies = []
    for i, b in enumerate(scene["bodies"]):
        pos = " ".join(str(p) for p in b["pos"])
        quat = " ".join(str(q) for q in quat_wxyz(b["orientation"]))
        inertia = " ".join(str(v) for v in b["inertia"])
        joint = "" if b["fixed"] else "<freejoint/>"
        inertial = "" if b["fixed"] else f'<inertial pos="0 0 0" mass="{b["mass"]}" diaginertia="{inertia}"/>'
        bodies.append(
            f'<body name="b{i}" pos="{pos}" quat="{quat}">{joint}{inertial}{geom(b["shape"], b["material"])}</body>'
        )
    return (
        '<mujoco model="oracle"><option gravity="0 0 0"/>'
        f'<worldbody>{"".join(bodies)}</worldbody></mujoco>'
    )


def main():
    directory, name = sys.argv[1], sys.argv[2]
    with open(os.path.join(directory, f"{name}.scene.json")) as f:
        scene = json.load(f)
    gs.init(backend=gs.cpu, precision="64", logging_level="warning")
    world = gs.Scene(
        sim_options=gs.options.SimOptions(dt=scene["dt"], gravity=(0.0, 0.0, 0.0)),
        rigid_options=gs.options.RigidOptions(
            friction_cone=gs.friction_cone.elliptic,
            enable_self_collision=True,
        ),
        show_viewer=False,
    )
    with tempfile.NamedTemporaryFile("w", suffix=".xml", delete=False) as f:
        f.write(mjcf(scene))
        path = f.name
    entity = world.add_entity(gs.morphs.MJCF(file=path))
    world.build()
    free = [i for i, b in enumerate(scene["bodies"]) if not b["fixed"]]
    links = {i: entity.get_link(f"b{i}") for i in range(len(scene["bodies"]))}
    velocity = []
    for i in free:
        b = scene["bodies"][i]
        velocity += list(b["vel"]) + list(b["omega_world"])
    entity.set_dofs_velocity(np.array(velocity))
    # Genesis's free-joint angular dofs may be body-frame; check and convert.
    for i in free:
        b = scene["bodies"][i]
        got = np.asarray(links[i].get_ang()).reshape(-1)[:3]
        if not np.allclose(got, b["omega_world"], atol=1e-9):
            local = rotate(quat_wxyz(b["orientation"]), b["omega_world"], inverse=True)
            velocity[6 * free.index(i) + 3 : 6 * free.index(i) + 6] = list(local)
            entity.set_dofs_velocity(np.array(velocity))
    samples = []
    for tick in range(1, scene["steps"] + 1):
        world.step()
        bodies = []
        for i, b in enumerate(scene["bodies"]):
            link = links[i]
            pos = np.asarray(link.get_pos()).reshape(-1)[:3]
            q = np.asarray(link.get_quat()).reshape(-1)[:4]
            vel = np.asarray(link.get_vel()).reshape(-1)[:3] if not b["fixed"] else np.zeros(3)
            ang = np.asarray(link.get_ang()).reshape(-1)[:3] if not b["fixed"] else np.zeros(3)
            bodies.append(
                {
                    "pos": pos.tolist(),
                    "vel": vel.tolist(),
                    "orientation": [q[1], q[2], q[3], q[0]],
                    "omega": rotate(q, ang, inverse=True).tolist(),
                }
            )
        samples.append({"tick": tick, "bodies": bodies})
    out = os.path.join(directory, f"{name}.genesis.json")
    with open(out, "w") as f:
        json.dump({"samples": samples}, f)
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
