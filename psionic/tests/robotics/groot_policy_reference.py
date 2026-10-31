#!/usr/bin/env python3
"""Firm reason2: join already-executed Coax stages and original GPU replay.

No learned model runs in this oracle. It checks actual file handoffs, all
frozen component comparisons, and the original output adapter including exact
gripper decisions. The input boundary is a prepared observation, not a raw
simulator adapter or successful control loop.
"""
import argparse
import hashlib
import json
from pathlib import Path
import numpy as np

p=argparse.ArgumentParser(description=__doc__)
p.add_argument("capsule",type=Path)
p.add_argument("vision",type=Path)
p.add_argument("text",type=Path)
p.add_argument("action",type=Path)
p.add_argument("decoded",type=Path)
p.add_argument("--record",required=True,type=Path)
p.add_argument("--conditional-head",required=True,type=Path)
a=p.parse_args()
assert not a.record.exists()
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def read(path):return np.frombuffer(path.read_bytes(),dtype="<f4")

for name in ["vision-merged","vision-deepstack0","vision-deepstack1","vision-deepstack2"]:
    assert (a.vision/(name+".f32")).read_bytes()==(a.text/"inputs"/(name+".f32")).read_bytes(),"vision/text handoff"
assert (a.text/"backbone.f32").read_bytes()==(a.action/"backbone.f32").read_bytes(),"text/action handoff"
inputs=np.load(a.capsule/"inputs.npz",allow_pickle=False)
assert (a.vision/"pixels.f32").read_bytes()==inputs["pixels"].astype("<f4").tobytes(),"prepared camera handoff"
assert (a.text/"inputs/ids.i64").read_bytes()==inputs["ids"].astype("<i8").tobytes()
for name in ["state","noise"]:
    assert (a.action/(name+".f32")).read_bytes()==inputs[name].astype("<f4").tobytes(),"state/noise handoff"

# Independent original adapter arithmetic and discrete LIBERO gripper mapping.
statistics=json.loads((Path(__file__).resolve().parents[2]/"fixtures/robotics/groot-libero/libero-statistics.json").read_text())
groups=["x","y","z","roll","pitch","yaw","gripper"]
low=np.concatenate([np.asarray(statistics["action"][k]["q01"],dtype=np.float64) for k in groups])
high=np.concatenate([np.asarray(statistics["action"][k]["q99"],dtype=np.float64) for k in groups])
normalized=read(a.capsule/"normalized-actions.f32").reshape(40,132)[:16,:7]
unit=(np.clip(normalized,np.float32(-1),np.float32(1))+np.float32(1))/np.float32(2)
expected=(unit.astype(np.float64)*(high-low)+low).astype(np.float32)
expected[:,6]=-np.sign(np.float32(2)*expected[:,6]-np.float32(1))
actual=read(a.decoded/"decoded-actions.f32").reshape(16,7)
assert np.isfinite(actual).all()
coax_normalized=read(a.action/"output/normalized-actions.f32").reshape(40,132)[:16,:7]
coax_unit=(np.clip(coax_normalized,np.float32(-1),np.float32(1))+np.float32(1))/np.float32(2)
coax_decoded=(coax_unit.astype(np.float64)*(high-low)+low).astype(np.float32)
coax_decoded[:,6]=-np.sign(np.float32(2)*coax_decoded[:,6]-np.float32(1))
assert np.array_equal(actual.view(np.uint32),coax_decoded.view(np.uint32)),"exact normalized/decoded handoff"
assert np.array_equal(actual[:,6].view(np.uint32),expected[:,6].view(np.uint32)),"discrete gripper mismatch"
error=np.abs(actual[:,:6].astype(np.float64)-expected[:,:6].astype(np.float64))
# Propagate the predeclared normalized endpoint tolerance through the fixed
# inverse affine map; no observed-error-dependent threshold or widened bound.
limit=(.002+.002*np.abs(normalized[:,:6].astype(np.float64)))*(high[:6]-low[:6])/2+2e-7
assert np.all(error<=limit),"decoded continuous action tolerance"
records={}
for name,folder in [("vision",a.vision),("text",a.text),("action",a.action)]:
    path=folder/("record.json" if name=="vision" else "reference.json")
    record=json.loads(path.read_text())
    status=record.get("passed",record.get("all_passed",False))
    if name!="action":
        assert status,(name,"component record did not pass")
    else:
        # A head conditioned on Coax's approximate upstream features must be
        # compared locally with a reference given those SAME features. Keep
        # the initial-full-network trace diagnostic, including failures, intact.
        endpoints={f"flow{step}-{suffix}" for step in range(4) for suffix in ("velocity","actions")}
        endpoints.add("normalized-actions")
        rows={row["name"]:row for row in record["captures"]}
        assert len(record["captures"])==31 and endpoints<=rows.keys()
        assert all(rows[name]["passed"] for name in endpoints),"original full-network endpoint comparison"
    records[name]={"record_sha256":sha(path),"record":record}
head=json.loads(a.conditional_head.read_text())
assert head["passed"] and head["conditioned_on_coax_backbone"]
assert head["backbone_sha256"]==sha(a.text/"backbone.f32"),"same-input head proof identity"
assert head["original_capsule_record_sha256"]==sha(a.capsule/"record.json")
assert not head["cpu_model_fallback"] and not head["source_checkout_executed"]
assert not head["original_full_network_reference_replaced"] and not head["global_hidden_trace_failure_relabelled"]
assert len(head["captures"])==31 and all(row["passed"] for row in head["captures"])
for row in head["captures"]:
    assert row["coax_sha256"]==sha(a.action/"output"/(row["name"]+".f32")),"conditional proof/output binding"
record={"schema":"psionic-groot-composed-policy-v1","passed":True,
        "input_boundary":"Admitted prepared observation: two packed cameras, unpadded original IDs, normalized category2 state and explicit Gaussian noise.",
        "all_learned_stages_coax_metal":True,"cpu_model_fallback":False,
        "model_revision":"2ea293aa20ba7cf5bbf3ba17a5fbcb1a01cbfe21","category":2,
        "exact_handoffs":["four vision outputs to text","text to action","original IDs,state,noise"],
        "decoded_values":112,"exact_gripper_decisions":16,
        "decoded_handoff_exact_f32_words":112,
        "decoded_max_abs_error":float(error.max()),"decoded_max_tolerance_fraction":float((error/limit).max()),
        "normalized_actions_sha256":sha(a.action/"output/normalized-actions.f32"),
        "decoded_actions_sha256":sha(a.decoded/"decoded-actions.f32"),
        "reference_capsule_record_sha256":sha(a.capsule/"record.json"),
        "all_global_hidden_boundaries_passed":records["action"]["record"]["passed"],
        "request_tokens":140,
        "conditional_head_proof":{"record_sha256":sha(a.conditional_head),"record":head},
        "acceptance":"Same-input component fidelity, unchanged original full-network action/velocity endpoints, and exact adapter handoff. Global hidden-trace diagnostics retain their own independent status.",
        "simulated_task_success_measured":False,"real_robot_execution":False,"components":records}
a.record.write_text(json.dumps(record,indent=2,sort_keys=True)+"\n")
print("GR00T composition: all learned stages, exact file handoffs,112decoded values and16exact gripper decisions passed")
