#!/usr/bin/env python3
"""Firm reason2: independent installed MuJoCo3.3.1 effects/replay oracle.

No modules from LIBERO, robosuite or NVIDIA checkouts are imported. Rendering
uses nonvisible CGL and is separate from the exact physics replay claim.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import xml.etree.ElementTree as ET
os.environ["MUJOCO_GL"] = "cgl"
import mujoco
import numpy as np


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def restore(model, data, state):
    mujoco.mj_resetData(model, data)
    data.time = state[0]
    data.qpos[:] = state[1:1+model.nq]
    data.qvel[:] = state[1+model.nq:1+model.nq+model.nv]
    mujoco.mj_forward(model, data)


def state(model, data):
    return np.concatenate(([data.time], data.qpos, data.qvel, data.act))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("xml", type=Path)
    parser.add_argument("initial", type=Path)
    parser.add_argument("prefix", type=Path)
    parser.add_argument("--render", type=Path)
    parser.add_argument("--record", type=Path, required=True)
    args = parser.parse_args()
    assert mujoco.__version__ == "3.3.1"
    raw = args.xml.read_text()
    assert "<!DOCTYPE" not in raw and "<!ENTITY" not in raw
    xml = ET.fromstring(raw)
    assert not list(xml.iter("include")) and not list(xml.iter("plugin"))
    model = mujoco.MjModel.from_xml_path(str(args.xml.resolve()))
    assert (model.nq, model.nv, model.nu, model.na) == (23, 21, 9, 0)
    assert model.opt.timestep == .002
    data = mujoco.MjData(model)
    initial = np.fromfile(args.initial, dtype="<f8")
    assert initial.size == 45
    controls = np.fromfile(str(args.prefix)+".controls.f64", dtype="<f8").reshape(25, 9)
    states = np.fromfile(str(args.prefix)+".states.f64", dtype="<f8").reshape(26, 45)
    restore(model, data, initial)
    joint_ids = [mujoco.mj_name2id(model, mujoco.mjtObj.mjOBJ_JOINT, f"robot0_joint{i}") for i in range(1, 8)]
    qids = model.jnt_qposadr[joint_ids]
    vids = model.jnt_dofadr[joint_ids]
    arm = [mujoco.mj_name2id(model, mujoco.mjtObj.mjOBJ_ACTUATOR, f"robot0_torq_j{i}") for i in range(1, 8)]
    fingers = [mujoco.mj_name2id(model, mujoco.mjtObj.mjOBJ_JOINT, f"gripper0_finger_joint{i}") for i in range(1, 3)]
    fids = model.jnt_qposadr[fingers]
    fac = [mujoco.mj_name2id(model, mujoco.mjtObj.mjOBJ_ACTUATOR, f"gripper0_gripper_finger_joint{i}") for i in range(1, 3)]
    site = mujoco.mj_name2id(model, mujoco.mjtObj.mjOBJ_SITE, "gripper0_grip_site")
    body = mujoco.mj_name2id(model, mujoco.mjtObj.mjOBJ_BODY, "robot0_right_hand")
    target = mujoco.mj_name2id(model, mujoco.mjtObj.mjOBJ_SITE, "desk_caddy_1_back_contain_region")
    book = mujoco.mj_name2id(model, mujoco.mjtObj.mjOBJ_BODY, "black_book_1_main")
    words = 0

    def exact(actual, expected, label):
        nonlocal words
        a, b = np.asarray(actual).ravel(), np.asarray(expected).ravel()
        assert a.dtype == b.dtype and a.shape == b.shape
        assert np.array_equal(a.view(np.uint64), b.view(np.uint64)), label
        words += a.size

    def telemetry():
        mass = np.zeros((model.nv, model.nv))
        jp, jr = np.zeros((3, model.nv)), np.zeros((3, model.nv))
        vel = np.zeros(6)
        mujoco.mj_fullM(model, mass, data.qM)
        mujoco.mj_jacSite(model, data, jp, jr, site)
        mujoco.mj_objectVelocity(model, data, mujoco.mjtObj.mjOBJ_SITE, site, vel, 0)
        return np.concatenate(([data.time], data.qpos[qids], data.qvel[vids],
                               mass[np.ix_(vids, vids)].ravel(), jp[:, vids].ravel(), jr[:, vids].ravel(),
                               data.site_xpos[site], data.site_xmat[site], vel[3:], vel[:3],
                               data.xquat[body], data.qpos[fids], model.actuator_ctrlrange[fac, 0],
                               model.actuator_ctrlrange[fac, 1], data.qfrc_bias[vids],
                               model.actuator_ctrlrange[arm, 0], model.actuator_ctrlrange[arm, 1],
                               data.site_xpos[target], data.site_xmat[target], model.site_size[target], data.xpos[book]))

    exact(state(model, data), states[0], "initial state")
    exact(telemetry(), np.fromfile(str(args.prefix)+".initial-telemetry.f64", dtype="<f8"), "initial telemetry")
    for i, control in enumerate(controls):
        mujoco.mj_forward(model, data)
        data.ctrl[arm+fac] = control
        mujoco.mj_step(model, data)
        exact(state(model, data), states[i+1], f"physics state{i+1}")
    stale = telemetry()
    exact(stale, np.fromfile(str(args.prefix)+".stale-telemetry.f64", dtype="<f8"), "stale telemetry")
    w = np.clip(data.xquat[body, 0], -1., 1.)
    den = np.sqrt(1-w*w)
    axis = np.zeros(3) if math.isclose(den, 0.) else (data.xquat[body, 1:]*2*math.acos(w))/den
    expected = np.concatenate((data.site_xpos[site], axis, data.qpos[fids])).astype(np.float32)
    actual = np.fromfile(str(args.prefix)+".observation.f32", dtype="<f4")
    assert np.array_equal(actual.view(np.uint32), expected.view(np.uint32)), "Coax observation"
    mujoco.mj_forward(model, data)
    exact(telemetry(), np.fromfile(str(args.prefix)+".fresh-telemetry.f64", dtype="<f8"), "fresh telemetry")
    image_bytes = 0
    if args.render:
        # A separate fresh SDK data object: rendering is outside physics replay.
        render_data = mujoco.MjData(model)
        restore(model, render_data, initial)
        option = mujoco.MjvOption()
        option.geomgroup[0] = 0
        option.geomgroup[1] = 1
        with mujoco.Renderer(model, 256, 256) as renderer:
            for name, suffix in [("agentview", "agentview"), ("robot0_eye_in_hand", "wrist")]:
                renderer.update_scene(render_data, camera=name, scene_option=option)
                # SDK Renderer adds flipud; native raw mjr_readPixels does not.
                raw_rgb = renderer.render()[::-1].copy()
                original = np.fromfile(str(args.render)+f".{suffix}.rgb", dtype="u1").reshape(256, 256, 3)
                assert np.array_equal(raw_rgb, original), name+" RGB"
                image_bytes += original.size
    record = {"schema": "psionic-panda-sdk-effects-reference-v1", "passed": True,
              "sdk": mujoco.__version__, "source_checkout_executed": False,
              "physics_steps": 25, "snapshots": 26, "exact_f64_words": words,
              "exact_observation_f32_words": 8, "separate_exact_render_bytes": image_bytes,
              "renderer_in_physics_replay": False, "window_created": False,
              "controls": "Actual Coax zero-pose/grip-hold controller output; independent SDK replays exact actuator values.",
              "xml_sha256": digest(args.xml), "initial_state_sha256": digest(args.initial),
              "controls_sha256": digest(Path(str(args.prefix)+".controls.f64")),
              "states_sha256": digest(Path(str(args.prefix)+".states.f64")),
              "source_sha256": digest(Path(__file__))}
    args.record.write_text(json.dumps(record, indent=2)+"\n")
    print(json.dumps(record, sort_keys=True))


if __name__ == "__main__":
    main()
