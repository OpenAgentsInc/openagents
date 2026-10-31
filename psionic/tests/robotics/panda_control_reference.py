#!/usr/bin/env python3
"""Independent NumPy source-formula oracle; never imports downloaded source.
Controller tolerances were fixed before observing native torques. Tooling only.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import platform
import struct
import sys
import numpy as np

ATOL, RTOL = 2e-5, 2e-6
MATRIX_ATOL, MATRIX_RTOL = 1e-10, 1e-9
ROWS = [0, 1, 2, 3, 64, 128, 232]
HOME = np.array([0, np.pi/16, 0, -np.pi/2-np.pi/3, 0, np.pi-.2, np.pi/4])
ACTIONS = [np.zeros(7), np.array([.2,-.3,.4,.1,-.2,.3,1]),
           np.array([-1,1,-1,-.6,.7,-.4,-1]), np.array([2,-2,.5,2,-2,0,0]),
           np.array([0,0,0,0,0,1e-12,.5]), np.array([0,0,0,0,0,0,4]),
           np.array([-.25,.75,-.8,1e-200,0,0,-.5]), np.array([1e100,-1e100,0,-1e100,1e100,0,1])]


def require(ok, message):
    if not ok:
        raise RuntimeError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def unpack_sample(raw):
    values = np.frombuffer(raw, dtype="<f8")
    require(values.size == 144, "PcSample144F64")
    lengths = [("position",3),("rotation",9),("q",7),("qd",7),("mass",49),("jp",21),("jr",21),("velocity",6),("bias",7),("lower",7),("upper",7)]
    result, cursor = {}, 0
    for key, size in lengths:
        result[key] = values[cursor:cursor+size].copy()
        cursor += size
    for key, shape in [("rotation",(3,3)),("mass",(7,7)),("jp",(3,7)),("jr",(3,7))]:
        result[key] = result[key].reshape(shape)
    return result


def quaternion_matrix(delta):
    angle = np.linalg.norm(delta)
    q = np.zeros(4)
    if math.isclose(angle, 0.0):
        q[3] = 1
    else:
        q[:3] = delta / angle * np.sin(angle/2)
        q[3] = np.cos(angle/2)
    # Pinned transform_utils.quat2mat explicitly narrows/reorders toF32.
    q = q.astype(np.float32)[[3,0,1,2]]
    norm = np.dot(q, q)
    if norm < np.finfo(float).eps * 4:
        return np.eye(3)
    q *= math.sqrt(2.0 / norm)
    t = np.outer(q, q)
    return np.array([[1.0-t[2,2]-t[3,3],t[1,2]-t[3,0],t[1,3]+t[2,0]],
                     [t[1,2]+t[3,0],1.0-t[1,1]-t[3,3],t[2,3]-t[1,0]],
                     [t[1,3]-t[2,0],t[2,3]+t[1,0],1.0-t[1,1]-t[2,2]]])


def source_case(sample, home, prime, action, use_prime, ticks):
    position = sample["position"].copy()
    rotation = sample["rotation"].copy()
    grip = np.zeros(2)
    mass = sample["mass"]
    jacobian = np.vstack((sample["jp"], sample["jr"]))
    inverse_mass = np.linalg.inv(mass)
    inertia_full = np.linalg.pinv(jacobian @ inverse_mass @ jacobian.T)
    inertia_pos = np.linalg.pinv(sample["jp"] @ inverse_mass @ sample["jp"].T)
    inertia_ori = np.linalg.pinv(sample["jr"] @ inverse_mass @ sample["jr"].T)
    null = np.eye(7) - (inverse_mass @ jacobian.T @ inertia_full) @ jacobian
    sequence = [(prime, True)] if use_prime else []
    sequence += [(action, tick == 0) for tick in range(ticks)]
    for command, new_goal in sequence:
        clipped = 0
        if new_goal:
            delta = np.clip(command[:6], -1, 1) * np.array([.05,.05,.05,.5,.5,.5])
            position = sample["position"] + delta[:3]
            if any(not math.isclose(x, 0.0) for x in delta[3:]):
                rotation = quaternion_matrix(delta[3:]) @ sample["rotation"]
            clipped = int(np.count_nonzero(np.abs(command[:6]) > 1))
        error = .5 * sum((np.cross(sample["rotation"][:,i], rotation[:,i]) for i in range(3)), np.zeros(3))
        force = 150*(position-sample["position"]) - 2*np.sqrt(150)*sample["velocity"][:3]
        torque = 150*error - 2*np.sqrt(150)*sample["velocity"][3:]
        wrench = np.concatenate((inertia_pos @ force, inertia_ori @ torque))
        raw = jacobian.T @ wrench + sample["bias"]
        raw += null.T @ (mass @ (10*(home-sample["q"]) - 2*np.sqrt(10)*sample["qd"]))
        torque_clipped = int(np.count_nonzero((raw < sample["lower"]) | (raw > sample["upper"])))
        torque = np.clip(raw, sample["lower"], sample["upper"])
        grip = np.clip(grip + np.array([-1.,1.])*.01*np.sign(command[6]), -1, 1)
        fingers = np.array([.02,-.02]) + .02*grip
    flat = np.concatenate((torque,fingers,position,rotation.reshape(-1),grip,home))
    return flat, [torque_clipped,clipped]


def matrix_cases():
    rng = np.random.default_rng(169)
    cases = []
    for n in range(1,8):
        for _ in range(5):
            x = rng.normal(size=(n,n))
            a = x @ x.T + .3*np.eye(n)
            cases.append((n,0,a))
    for n in (3,6):
        for _ in range(10):
            cases.append((n,1,rng.normal(size=(n,n))+2*np.eye(n)))
        for rank in range(1,n):
            x = rng.normal(size=(n,rank));y = rng.normal(size=(n,rank))
            cases.append((n,1,x @ y.T))
        for tiny in (0.,np.nextafter(1e-15,0),1e-15,np.nextafter(1e-15,np.inf),2e-15,5e-16):
            values = np.linspace(1.,.1,n);values[-1]=tiny
            cases.append((n,1,np.diag(values)))
        cases.append((n,1,np.zeros((n,n))))
        for scale in (1e-150,1e150):
            cases.append((n,1,np.eye(n)*scale))
    return cases


def prepare(fixture, work):
    work.mkdir(parents=True, exist_ok=False)
    cases, source = [], []
    for row in ROWS:
        raw = (fixture / f"row{row}.pc-sample.f64").read_bytes()
        sample = unpack_sample(raw)
        source.append({"row":row,"bytes":len(raw),"sha256":sha(raw)})
        for index, action in enumerate(ACTIONS):
            for ticks in (1,25):
                use_prime = index % 2
                prime = np.array([.1,-.2,.3,.25,-.15,.2,-1.])
                cases.append((raw,sample,HOME.copy(),prime,action,use_prime,ticks))
    with (work/"controller-input.bin").open("xb") as stream:
        stream.write(struct.pack("<II",0x50433136,len(cases)))
        for raw,sample,home,prime,action,use_prime,ticks in cases:
            stream.write(raw);stream.write(np.concatenate((home,prime,action)).astype("<f8").tobytes());stream.write(struct.pack("<II",use_prime,ticks))
    matrices = matrix_cases()
    with (work/"matrix-input.bin").open("xb") as stream:
        stream.write(struct.pack("<II",0x504d3136,len(matrices)))
        for n,operation,a in matrices:
            padded=np.zeros((7,7));padded[:n,:n]=a
            stream.write(struct.pack("<II",n,operation));stream.write(padded.astype("<f8").tobytes())
    (work/"inputs.json").write_text(json.dumps({"source_samples":source,"controller_cases":len(cases),"matrix_cases":len(matrices),"home":HOME.tolist(),"scope":"Seven frozen source-demo0 telemetry frames; owned action perturbations and fixed-state controller replay only."},indent=2)+"\n")
    print(f"Prepared {len(cases)} independent source-controller cases and {len(matrices)} smallmatrix cases")


def compare(fixture, work):
    info=json.loads((work/"inputs.json").read_text());expected=[];clips=[]
    for row in ROWS:
        sample=unpack_sample((fixture/f"row{row}.pc-sample.f64").read_bytes())
        for index,action in enumerate(ACTIONS):
            for ticks in (1,25):
                value,count=source_case(sample,HOME,np.array([.1,-.2,.3,.25,-.15,.2,-1.]),action,index%2,ticks)
                expected.append(value);clips.append(count)
    expected=np.asarray(expected);clips=np.asarray(clips);records=[];passed=True;first_controller=None;first_matrix=None
    for mode in ("debug","release","sanitized"):
        raw=(work/(mode+".controller.bin")).read_bytes();require(len(raw)==len(expected)*252,"controller output extent")
        observed=[];counts=[]
        for index in range(len(expected)):
            piece=raw[index*252:(index+1)*252];require(struct.unpack_from("<I",piece)[0]==0,"controller refusal")
            observed.append(np.frombuffer(piece,dtype="<f8",count=30,offset=4));counts.append(struct.unpack_from("<II",piece,244))
        observed=np.asarray(observed);require(np.isfinite(observed).all(),"nonfinite controller output")
        torque_error=np.abs(observed[:,:7]-expected[:,:7]);budget=ATOL+RTOL*np.abs(expected[:,:7]);torques=bool((torque_error<=budget).all())
        rotation_error=float(np.abs(observed[:,12:21]-expected[:,12:21]).max());remaining=np.r_[7:12,21:30];other_error=float(np.abs(observed[:,remaining]-expected[:,remaining]).max());count_match=np.array_equal(np.asarray(counts),clips)
        matrix_raw=(work/(mode+".matrix.bin")).read_bytes();matrices=matrix_cases();require(len(matrix_raw)==len(matrices)*400,"matrix output extent");matrix_pass=True;matrix_error=0.;rank_pass=True
        for index,(n,operation,a) in enumerate(matrices):
            piece=matrix_raw[index*400:(index+1)*400];status,rank=struct.unpack_from("<II",piece);require(status==0,f"matrix refusal {index}: {status}")
            actual=np.frombuffer(piece,dtype="<f8",count=49,offset=8).reshape(7,7)[:n,:n];target=np.linalg.pinv(a) if operation else np.linalg.inv(a)
            singular=np.linalg.svd(a,compute_uv=False);target_rank=int((singular>1e-15*singular.max(initial=0)).sum()) if operation else n;rank_pass &= rank==target_rank
            error=np.abs(actual-target);matrix_pass &= bool((error<=MATRIX_ATOL+MATRIX_RTOL*np.abs(target)).all());relative=np.linalg.norm(actual-target)/max(np.linalg.norm(target),1e-300);matrix_pass &= relative<=1e-9;matrix_error=max(matrix_error,float(relative))
        mode_pass=torques and rotation_error<=5e-7 and other_error<=2e-12 and count_match and matrix_pass and rank_pass;passed &= mode_pass
        if first_controller is None:first_controller=raw;first_matrix=matrix_raw
        identical=raw==first_controller and matrix_raw==first_matrix;passed &= identical
        records.append({"mode":mode,"passed":mode_pass,"controller_cases":len(expected),"torque_max_abs_error":float(torque_error.max()),"torque_max_budget_fraction":float((torque_error/budget).max()),"goal_rotation_max_abs_error":rotation_error,"other_state_max_abs_error":other_error,"clip_counts_match":bool(count_match),"matrix_cases":len(matrices),"matrix_max_relative_frobenius_error":matrix_error,"matrix_ranks_match_default_numpy_pinv":bool(rank_pass),"native_modes_bit_identical":identical,"controller_sha256":sha(raw),"matrix_sha256":sha(matrix_raw)})
    record={"schema":"psionic-panda-controller-source-reference-v1","passed":bool(passed),"profile":"F64 OSC with source F32 quat2mat staging, fixed arrays and one-sidedJacobi pinv1e-15; independently authored NumPy1.26 source formulas, no clonedsource execution/Numba identity claim.","controller_tolerance_frozen_before_observation":{"atol":ATOL,"rtol":RTOL},"matrix_tolerance":{"atol":MATRIX_ATOL,"rtol":MATRIX_RTOL,"relative_frobenius":1e-9},"runtime":{"python":platform.python_version(),"numpy":np.__version__,"interpreter":sys.executable},"inputs":info,"modes":records,"scope":"Frozen controller numerical/source semantics and refusal ABI only; no simulator integration or closed-loop success claim."}
    with (work/"reference.json").open("x") as stream:json.dump(record,stream,indent=2,sort_keys=True);stream.write("\n")
    for result in records:print(result)
    require(passed,"controller/source comparison failed; inspect record without widening frozen limits")


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument("mode",choices=("prepare","compare"));parser.add_argument("fixture",type=Path);parser.add_argument("work",type=Path);args=parser.parse_args()
    (prepare if args.mode=="prepare" else compare)(args.fixture,args.work)


if __name__=="__main__":main()
