#!/usr/bin/env python3
# Firm reason: independent numerical oracle and artifact audit; tooling only.
"""Check the frozen residual CNN without training or modifying the MNIST MLP tooling.

Reuse authenticated MNIST bytes, pooling, RNG and generic artifact readers.
Convolution and its formal VJP are independent dense formulas. Forward products
and ordered sums round to f32; reverse accumulation is f64, published once as
f32. No optimizer or extra post-training forward pass runs in this tool.
"""
import argparse
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import struct

_source=Path(__file__).with_name("mnist_reference.py")
_spec=importlib.util.spec_from_file_location("cnn_mnist_helpers",_source)
mn=importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(mn)
ROOT=Path(__file__).resolve().parents[2]
FIXTURES=ROOT/"psionic/fixtures/residual-mlp"
DEFAULT_DATA=mn.DEFAULT_DATA
f32=mn.f32
sum_f32=mn.sum_f32
digest=mn.digest
verify_data=mn.verify_data
pool=mn.pool
initial_value=mn.initial_value
epoch_order=mn.epoch_order
f32_sha=mn.f32_sha
read_floats=mn.read_floats
compare_values=mn.compare_values
tagged_sha256=mn.tagged_sha256
check_pointer=mn.check_pointer
unique_json=mn.unique_json
score_metrics=mn.score_metrics
dataset_manifest=mn.dataset_manifest
TOLERANCES=mn.TOLERANCES
EXPORT_SHAPES={
    "block.expand.weight":([2,1,1,1],0,2),"block.expand.bias":([1,2,1,1],2,2),
    "block.contract.weight":([1,2,1,1],4,2),"block.contract.bias":([1,1,1,1],6,1),
    "fc1.weight":([196,16],7,3136),"fc1.bias":([16],3143,16),
    "fc2.weight":([16,10],3159,160),"fc2.bias":([10],3319,10)}


def initial_parameters():
    return ([initial_value(11821,k,1) for k in range(206,210)]+
            [initial_value(11821,k,2) for k in range(210,213)]+
            [initial_value(11821,k,196) for k in range(213,3365)]+
            [initial_value(11821,k,16) for k in range(3365,3535)])


def cnn(x,target,parameters):
    """Independent scalar residual/MLP-head equations; no optimizer code."""
    if len(x)!=196 or len(target)!=10 or len(parameters)!=3329:
        raise ValueError("residual MLP-head shape mismatch")
    if not all(math.isfinite(v) for v in (*x,*target,*parameters)):
        raise ValueError("residual MLP-head inputs must be finite")
    a,b,c,d=parameters[:2],parameters[2:4],parameters[4:6],parameters[6]
    w1,b1,w2,b2=parameters[7:3143],parameters[3143:3159],parameters[3159:3319],parameters[3319:]
    pre=[[f32(f32(v*a[h])+b[h]) for v in x] for h in range(2)]
    hidden=[[max(v,0.0) for v in row] for row in pre]
    z=[f32(f32(sum_f32(hidden[h][j]*c[h] for h in range(2))+d)+x[j]) for j in range(196)]
    q=[f32(sum_f32(z[j]*w1[j*16+h] for j in range(196))+b1[h]) for h in range(16)]
    hh=[max(v,0.0) for v in q]
    y=[f32(sum_f32(hh[h]*w2[h*10+k] for h in range(16))+b2[k]) for k in range(10)]
    error=[f32(y[k]-target[k]) for k in range(10)]
    loss=sum_f32(v*v for v in error);dy=[2*float(v) for v in error]
    dq=[sum(dy[k]*w2[h*10+k] for k in range(10)) if q[h]>0 else 0.0 for h in range(16)]
    dz=[sum(dq[h]*w1[j*16+h] for h in range(16)) for j in range(196)]
    dh=[[dz[j]*c[h] if pre[h][j]>0 else 0.0 for j in range(196)] for h in range(2)]
    ga=[sum(dh[h][j]*x[j] for j in range(196)) for h in range(2)]
    gb=[sum(row) for row in dh]
    gc=[sum(dz[j]*hidden[h][j] for j in range(196)) for h in range(2)];gd=sum(dz)
    gradient=([f32(v) for v in (*ga,*gb,*gc,gd)]+
              [f32(z[j]*dq[h]) for j in range(196) for h in range(16)]+[f32(v) for v in dq]+
              [f32(hh[h]*dy[k]) for h in range(16) for k in range(10)]+[f32(v) for v in dy])
    return y,loss,gradient,dict(branch=pre,head=q)


def oracle(data_dir):
    raw=verify_data(data_dir);p=initial_parameters();order=epoch_order(4096,11822,0);rows=[]
    for row in (0,order[0]):
        x=pool(raw["train_images"][16+row*784:16+(row+1)*784]);label=raw["train_labels"][8+row]
        y,loss,g,pre=cnn(x,[float(i==label) for i in range(10)],p)
        probes=[0,1,4,7,1023,1966,1967,3142,3143,3158,3159,3318,3319,3328]
        rows.append(dict(source_index=row,label=label,x_sha256=f32_sha(x),logits=y,
            sum_squared_error=loss,preactivation=pre,gradient_f32_sha256=f32_sha(g),
            residual_gradient_f32_sha256=f32_sha(g[:7]),
            nonzero_residual_gradients=sum(v!=0 for v in g[:7]),
            gradient_probes={str(i):g[i] for i in probes}))
    return dict(schema=1,scope="fixed CNN initialization/forward/formal gradient; no optimizer",
        arithmetic="ordered C,KH,KW f32 products and sequential sums; f64 VJP accumulation; f32 published entry gradients",
        shape=dict(input=[1,1,14,14],expand=[2,1,1,1],contract=[1,2,1,1],activation=[1,1,14,14],flatten=196,classes=10),
        parameters=3329,inputs=3536,prediction_node=21,tolerances=TOLERANCES,
        initializer_seed=11821,parameters_f32_sha256=f32_sha(p),
        epoch0_order_sha256=digest(struct.pack("<4096q",*order)),epoch0_first16=order[:16],rows=rows)


def c_header(data_dir):
    raw=verify_data(data_dir);p=initial_parameters();label=raw["train_labels"][8]
    x=pool(raw["train_images"][16:800]);y,loss,g,_=cnn(x,[float(i==label) for i in range(10)],p)
    lines=["/* Independent frozen residual CNN reference; no optimizer execution. */",
        "static const int cnn_reference_row = 0;",f"static const int cnn_reference_label = {label};",
        f"static const float cnn_reference_loss = {loss.hex()}f;"]
    for name,values in (("input",x),("parameters",p),("logits",y),("gradient",g)):
        lines.append(f"static const float cnn_reference_{name}[{len(values)}] = {{"+
                     ",".join(float(v).hex()+"f" for v in values)+"};")
    return "\n".join(lines)+"\n"


def verify_capture(capture_dir,data_dir):
    metadata=json.loads((capture_dir/"oracle.json").read_text(),object_pairs_hook=unique_json)
    status=metadata.get("status")
    if not ((type(status)is int and status==0) or status=="ok"):raise ValueError("native oracle status is not ok")
    for key,want in (("parameters",3329),("inputs",3536)):
        if type(metadata.get(key))is not int or metadata[key]!=want:raise ValueError(f"native oracle {key} count mismatch")
    for key in ("values","adjoints","flags"):
        if type(metadata.get(key))is not int or metadata[key]<1:raise ValueError(f"native oracle {key} must be positive")
    loss=metadata.get("loss")
    if type(loss)not in (int,float) or not math.isfinite(loss):raise ValueError("native oracle loss is not finite")
    raw=verify_data(data_dir);p=initial_parameters();x=pool(raw["train_images"][16:800]);label=raw["train_labels"][8]
    y,want_loss,g,_=cnn(x,[float(k==label) for k in range(10)],p);checks={}
    for name,filename,want,code in (("parameters","params.f64",p,"d"),("gradient","gradient.f64",g,"d"),
                                  ("scores","scores.f32",y,"f"),("features","features.f32",x,"f")):
        checks[name]=compare_values(name,read_floats(capture_dir/filename,len(want),code),want,code)
    checks["loss"]=compare_values("loss",[loss],[want_loss],"f")
    return dict(verified=True,source="original official TRAIN row0",label=label,checks=checks,
        observed_native_counts={k:metadata[k] for k in ("parameters","inputs","values","adjoints","flags")},
        dataset_manifest_sha256=digest((mn.FIXTURES/"dataset.json").read_bytes()),
        experiment_manifest_sha256=digest((FIXTURES/"experiment.json").read_bytes()),
        scope="public residual CNN forward and all 3329 formal parameter gradients; no optimizer or quality claim")


def checkpoint_identities():
    """Independently serialize the frozen public trainer configuration."""
    bits = lambda x: struct.unpack("<Q", struct.pack("<d",x))[0]
    words = ([1,0,0,0] + [bits(x) for x in (0.003,0.0,0.9,0.999,1e-8,0.0)] +
             [0]*9 + [bits(1.0),16,bits(16.0),4096,1,12,1,11822,0,0,0,0])
    if len(words) != 31:
        raise ValueError("frozen trainer identity must contain 31 words")
    optimizer = tagged_sha256("psionic.trainer-config.v1",struct.pack("<31Q",*words))
    prep = b"mnist-idx-pool-v1:28x28;2x2-average;sum/1020;f32;first4096;next1024;official-test-first2048"
    objects = dataset_manifest()["objects"]
    data = b"".join(bytes.fromhex(x["decompressed_sha256"]) for x in objects)
    manifest=hashlib.sha256((FIXTURES/"experiment.json").read_bytes()).digest()
    initial=struct.pack("<3329d",*initial_parameters())
    model=tagged_sha256("psionic.residual-mlp-initial-model.v1",manifest+initial)
    return (model,
            hashlib.sha256(prep).digest(),hashlib.sha256(data).digest(),optimizer)


def parse_checkpoint(raw, identities, revision):
    n = 3329
    body = 228+24*n
    if len(raw) != body+32+80:
        raise ValueError("checkpoint extent mismatch")
    if raw[:8] != b"PSCK\x01\x00\x04\x00" or struct.unpack_from("<I",raw,224)[0] != n:
        raise ValueError("checkpoint header or parameter count mismatch")
    d = tagged_sha256("psionic.checkpoint.v1",raw[:body])
    if d != raw[body:body+32]:
        raise ValueError("checkpoint tagged digest mismatch")
    if tuple(raw[8+32*i:40+32*i] for i in range(4)) != tuple(identities):
        raise ValueError("checkpoint model/preprocess/dataset/optimizer identity mismatch")
    if raw[136:168] != bytes(32):
        raise ValueError("unexpected checkpoint parent")
    words = struct.unpack_from("<7Q",raw,168)
    expected = (revision,11822,revision//256,(revision%256)*16,0,revision,0)
    if words != expected:
        raise ValueError("checkpoint revision/cursor/optimizer counters mismatch")
    values = struct.unpack_from(f"<{3*n}d",raw,228)
    if not all(math.isfinite(x) for x in values) or any(x < 0 for x in values[2*n:]):
        raise ValueError("checkpoint non-finite value or negative second moment")
    if any(abs(x)>16 for x in values[:n]):
        raise ValueError("checkpoint parameter exceeds frozen bound")
    tail = raw[body+32:]
    if tail[:8] != b"PSTR\x01\x00\x00\x00":
        raise ValueError("trainer tail header mismatch")
    if tagged_sha256("psionic.trainer.v1",d+tail[8:48]) != tail[48:80]:
        raise ValueError("trainer tail tagged digest mismatch")
    evals,stale,last,stopped,best = struct.unpack_from("<4Qd",tail,8)
    if (evals>revision+1 or stale>evals or last>revision+1 or stopped>1 or
            not math.isfinite(best)):
        raise ValueError("invalid trainer tail fields")
    # This experiment's evaluations are external to tr_evaluate (every=0).
    if (evals,stale,last,stopped,best) != (0,0,0,0,0.0):
        raise ValueError("unexpected trainer evaluation/early-stop state")
    return dict(revision=revision,digest=d,parameters=list(values[:n]),
                first_moments=list(values[n:2*n]),second_moments=list(values[2*n:]),
                optimizer_identity=identities[3].hex())


def check_export(raw, master):
    if len(raw)<8:
        raise ValueError("short safetensors file")
    count = struct.unpack_from("<Q",raw)[0]
    if count<2 or count>16384 or 8+count>len(raw):
        raise ValueError("safetensors header extent mismatch")
    header = json.loads(raw[8:8+count],object_pairs_hook=unique_json)
    shapes = EXPORT_SHAPES
    if set(header) != set(shapes)|{"__metadata__"} or header["__metadata__"] != {"format":"psionic-interchange-v1"}:
        raise ValueError("safetensors tensor names/metadata mismatch")
    payload = raw[8+count:]
    ranges = []
    for name,(shape,first,n) in shapes.items():
        item = header[name]
        if set(item)!={"dtype","shape","data_offsets"} or item["dtype"]!="F32" or item["shape"]!=shape:
            raise ValueError(f"{name}: dtype/shape mismatch")
        offsets = item["data_offsets"]
        if (not isinstance(offsets,list) or len(offsets)!=2 or
                any(type(x)is not int for x in offsets)):
            raise ValueError(f"{name}: invalid payload offsets")
        lo,hi=offsets
        if lo<0 or hi-lo!=4*n or hi>len(payload):
            raise ValueError(f"{name}: payload extent mismatch")
        expected=b"".join(struct.pack("<f",p) for p in master[first:first+n])
        if payload[lo:hi]!=expected:
            raise ValueError(f"{name}: exported words differ from final master rounded to f32")
        ranges.append((lo,hi))
    ranges.sort()
    if ranges[0][0]!=0 or ranges[-1][1]!=len(payload) or any(a[1]!=b[0] for a,b in zip(ranges,ranges[1:])):
        raise ValueError("safetensors overlapping, missing or extra payload")
    return dict(tensors=8,words=3329,bytes=len(raw),all_exported_words_match=True)


def verify_run(run_dir,data_dir):
    result=json.loads((run_dir/"result.json").read_text(),object_pairs_hook=unique_json)
    if type(result.get("status"))is not int:
        raise ValueError("result status is not an integer")
    for key,want in (("steps",3072),("train_rows",49152),("test_rows",2048),("checkpoint_bytes",80236)):
        if type(result.get(key))is not int or result[key]!=want:
            raise ValueError(f"result {key} mismatch")
    for key in ("initial_loss","final_loss","seconds"):
        if type(result.get(key))not in (int,float) or not math.isfinite(result[key]) or result[key]<0:
            raise ValueError(f"result {key} must be finite and nonnegative")
    for key in ("initial_correct","final_correct","baseline_correct"):
        if type(result.get(key))is not int or not 0<=result[key]<=2048:
            raise ValueError(f"result {key} outside held-out extent")
    raw=verify_data(data_dir)
    histogram=[raw["train_labels"][8:8+4096].count(k) for k in range(10)]
    baseline=max(range(10),key=lambda k:histogram[k])
    if type(result.get("baseline_class"))is not int or result["baseline_class"]!=baseline:
        raise ValueError("baseline class was not the training-only first-tie majority")
    labels=raw["test_labels"][8:8+2048]
    initial_scores=read_floats(run_dir/"initial-test-scores.f32",20480,"f")
    initial_metrics=score_metrics(initial_scores,labels,baseline)
    scores=read_floats(run_dir/"test-scores.f32",20480,"f")
    metrics=score_metrics(scores,labels,baseline)
    if metrics["correct"]!=result["final_correct"] or metrics["baseline_correct"]!=result["baseline_correct"]:
        raise ValueError("reported final or majority correct count differs from official labels")
    if initial_metrics["correct"]!=result["initial_correct"]:
        raise ValueError("reported initial correct count differs from captured initial scores")
    compare_values("loss",[result["initial_loss"]],[initial_metrics["mean_sse"]],"d")
    compare_values("loss",[result["final_loss"]],[metrics["mean_sse"]],"d")
    trace=read_floats(run_dir/"validation.f64",52,"d")
    for epoch in range(13):
        e,loss,correct,step_loss=trace[epoch*4:epoch*4+4]
        if e!=epoch or loss<0 or correct!=int(correct) or not 0<=correct<=1024 or step_loss<0 or (epoch==0 and step_loss!=0):
            raise ValueError("validation trace extent/epoch/count/loss mismatch")
    ids=checkpoint_identities();checkpoints={}
    for stem,revision in (("epoch6",1536),("final",3072),("canonical-next",1537),("replayed-next",1537)):
        body=(run_dir/(stem+".psck")).read_bytes()
        checkpoints[stem]=parse_checkpoint(body,ids,revision)
        if stem in ("epoch6","final"):
            check_pointer((run_dir/(stem+".pscp")).read_bytes(),checkpoints[stem])
    if (run_dir/"canonical-next.psck").read_bytes()!=(run_dir/"replayed-next.psck").read_bytes():
        raise ValueError("resumed next checkpoint differs in parameters, moments, cursor or trainer tail")
    a=(run_dir/"canonical-order.i64").read_bytes();b=(run_dir/"replayed-order.i64").read_bytes()
    expected=struct.pack("<4096q",*epoch_order(4096,11822,6))
    if a!=b or a!=expected:
        raise ValueError("canonical/replayed order differs from epoch6 frozen shuffle")
    exported=(run_dir/"model.safetensors").read_bytes()
    export=check_export(exported,checkpoints["final"]["parameters"])
    if result.get("export_bytes")!=len(exported):
        raise ValueError("reported export extent mismatch")
    accuracy=metrics["correct"]/2048;baseline_accuracy=metrics["baseline_correct"]/2048
    failures=[]
    if result.get("status")!=0:failures.append("native run did not report ok")
    if accuracy<0.80:failures.append("test accuracy below frozen0.80")
    if accuracy-baseline_accuracy<0.50:failures.append("test accuracy margin over training-majority baseline below0.50")
    if not metrics["mean_sse"]<initial_metrics["mean_sse"]:failures.append("final test mean SSE did not fall below initial captured-score mean SSE")
    if result.get("replay_equal")is not True:failures.append("native replay_equal is not true")
    if result.get("export_equal")is not True:failures.append("native export_equal is not true")
    return dict(passed=not failures,failures=failures,test=dict(rows=2048,accuracy=accuracy,
        majority_class=baseline,majority_accuracy=baseline_accuracy,**metrics),
        initial=initial_metrics,
        validation=dict(rows=13,columns=["epoch","mean validation SSE","validation correct","last optimizer step mean16row training SSE"],
            scope="finite extent/epoch/count checks only; validation scores were not separately captured"),
        replay=dict(exact_checkpoint_bytes=True,exact_epoch6_order=True,revisions=[1536,1537,3072]),
        export=export,identities={k:v.hex() for k,v in zip(("model","preprocess","dataset","optimizer"),ids)},
        scope="initial/final metrics recomputed from scheduled evaluation score captures, no extra forward or training run; export-reload prediction equality is producer-recorded")


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command",choices=("oracle","verify-capture","verify-run"))
    parser.add_argument("capture_dir",nargs="?",type=Path)
    parser.add_argument("--data-dir",type=Path,default=DEFAULT_DATA)
    parser.add_argument("--out",type=Path)
    parser.add_argument("--header",type=Path)
    args=parser.parse_args()
    if args.command=="oracle":
        result=oracle(args.data_dir)
        if args.header:args.header.write_text(c_header(args.data_dir))
    else:
        if args.capture_dir is None:parser.error("a capture/run directory is required")
        result=(verify_capture if args.command=="verify-capture" else verify_run)(args.capture_dir,args.data_dir)
    text=json.dumps(result,indent=2,allow_nan=False)+"\n"
    if args.out:args.out.write_text(text)
    else:print(text,end="")
    if args.command=="verify-run" and not result["passed"]:raise SystemExit(1)


if __name__=="__main__":main()
