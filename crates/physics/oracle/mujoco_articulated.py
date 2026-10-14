"""Generate offline articulated-joint references with MuJoCo 3.3.7.

These original test models and their generated data are CC0-1.0.
The product and Cargo tests do not import or execute Python or MuJoCo.
"""

import hashlib
import json
import math
import pathlib
import sys

import mujoco

assert mujoco.__version__ == "3.3.7", "Use mujoco==3.3.7 for this reference"
DT = 1.0 / 240.0


def model_xml(count, kind, mass, size, kp, kd, effort, gravity):
    x, y, z = size
    inertia = [mass * (y*y + z*z)/12, mass * (x*x + z*z)/12, mass * (x*x + y*y)/12]
    bodies = []
    for i in range(count):
        position = "0 0 0" if i == 0 else f"{x} 0 0"
        center = "0 0 0" if count == 1 else f"{x/2} 0 0"
        bodies.append(
            f'<body name="link{i}" pos="{position}">'
            f'<joint name="joint{i}" type="{kind}" axis="0 0 1" limited="true" range="-1 1"/>'
            f'<inertial pos="{center}" mass="{mass}" diaginertia="{" ".join(map(str, inertia))}"/>'
        )
    actuators = ''.join(f'<position joint="joint{i}" kp="{kp}" kv="{kd}" forcelimited="true" forcerange="-{effort} {effort}"/>' for i in range(count))
    return (f'<mujoco model="articulated-reference"><compiler angle="radian"/>'
            f'<option timestep="{DT}" gravity="{gravity}" integrator="implicitfast" iterations="100" tolerance="1e-12"/>'
            f'<worldbody>{"".join(bodies)}{"</body>"*count}</worldbody>'
            f'<actuator>{actuators}</actuator></mujoco>')


def record(name, count, kind, mass, size, kp, kd, effort, speed, gravity, steps, target):
    xml = model_xml(count, kind, mass, size, kp, kd, effort, gravity)
    model = mujoco.MjModel.from_xml_string(xml)
    data = mujoco.MjData(model)
    rows = []
    for step in range(steps):
        actions = [target(step*DT, i) for i in range(count)]
        # Match the Rust servo's implicit PD speed command. MuJoCo's
        # implicit integrator handles velocity damping independently.
        for i, value in enumerate(actions):
            raw = kp*(value-data.qpos[i])/(kd+DT*kp)
            clipped = max(-speed, min(speed, raw))
            data.ctrl[i] = data.qpos[i] + clipped*(kd+DT*kp)/kp
        mujoco.mj_step(model, data)
        rows.append({"actions": actions, "state": data.qpos.tolist()})
    return {
        "name": name, "model_xml": xml, "model_sha256": hashlib.sha256(xml.encode()).hexdigest(),
        "count": count, "kind": kind, "mass": mass, "size": size,
        "kp": kp, "kd": kd, "effort": effort, "speed": speed,
        "gravity": list(map(float, gravity.split())), "steps": rows,
    }


cases = [
    record("hinge", 1, "hinge", 1.0, [math.sqrt(0.06)]*3, 4.0, 0.4, 2.4, 2.0, "0 0 0", 240, lambda t, i: 0.5),
    record("slider", 1, "slide", 1.0, [math.sqrt(0.6)]*3, 100.0, 20.0, 50.0, 0.5, "0 0 0", 480, lambda t, i: 0.3),
    record("six-link-chain", 6, "hinge", 0.1, [0.08, 0.02, 0.02], 4.0, 0.4, 2.4, 2.0, "0 -9.81 0", 720,
           lambda t, i: 0.12*math.sin(0.6*t + i*0.2)*min(1.0, t/0.5)),
]
output = {"schema": "physics.articulated-reference.v1", "mujoco": mujoco.__version__, "timestep": DT, "license": "CC0-1.0", "cases": cases}
pathlib.Path(sys.argv[1]).write_text(json.dumps(output, separators=(",", ":")) + "\n")
