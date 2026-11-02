#!/usr/bin/env python3
# Firm reason: independent data provenance and numerical oracle, never product runtime.
"""Authenticate MNIST and compute fixed forward/gradient references, never train.

The data source is the CVDF mirror authorized by Yann LeCun. Only its four
pinned gzip objects are fetched; no downloaded code is executed. All dataset
bytes stay in the selected external directory. Binary32 arithmetic uses struct
rounding after each product and sum; a small dyadic model is also checked
with exact rational arithmetic. There is no optimizer here.
Post-run audits inspect saved scores, checkpoint identities and exported words;
they perform no extra model forward pass or learning step.
"""
import argparse
import base64
from datetime import datetime, timezone
from fractions import Fraction
import gzip
import hashlib
import json
import math
from pathlib import Path
import struct
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "psionic/fixtures/mnist"
DEFAULT_DATA = Path("/tmp/psionic-mnist-data")
MASK64 = (1 << 64) - 1
# Declared before observing any native capture. Do not widen after a failure.
TOLERANCES = {
    "parameters": {"comparison": "exact binary64 bits"},
    "features": {"comparison": "exact binary32 bits"},
    "scores": {"absolute": 2e-6, "relative": 2e-6},
    "gradient": {"absolute": 2e-6, "relative": 2e-6},
    "loss": {"absolute": 2e-6, "relative": 2e-6},
}


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def sum_f32(values):
    total = 0.0
    for value in values:
        total = f32(total + f32(value))
    return total


def digest(data):
    return hashlib.sha256(data).hexdigest()


def check_digest(data, algorithm, expected):
    if hashlib.new(algorithm, data).hexdigest() != expected:
        raise ValueError(f"{algorithm} digest mismatch")


def check_idx(raw, item):
    if len(raw) != item["decompressed_bytes"]:
        raise ValueError("IDX length mismatch")
    images = item["kind"] == "images"
    header = struct.unpack(">IIII" if images else ">II", raw[:16 if images else 8])
    expected = (2051, item["records"], 28, 28) if images else (2049, item["records"])
    if header != expected:
        raise ValueError("IDX header mismatch")
    if not images and any(x > 9 for x in raw[8:]):
        raise ValueError("MNIST label outside 0..9")


def dataset_manifest():
    return json.loads((FIXTURES / "dataset.json").read_text())


def validate_object(packed, item):
    if len(packed) != item["gzip_bytes"]:
        raise ValueError("gzip length mismatch")
    check_digest(packed, "md5", item["gzip_md5"])
    check_digest(packed, "sha256", item["gzip_sha256"])
    # Bound decompression by the exact pinned length before publishing bytes.
    import io
    with gzip.GzipFile(fileobj=io.BytesIO(packed)) as stream:
        raw = stream.read(item["decompressed_bytes"] + 1)
    check_digest(raw, "sha256", item["decompressed_sha256"])
    check_idx(raw, item)
    return raw


def acquire(data_dir):
    data_dir.mkdir(parents=True, exist_ok=True)
    receipt = dict(schema=1, source_page="https://github.com/cvdfoundation/mnist",
                   retrieved_at=datetime.now(timezone.utc).isoformat(), objects=[])
    for item in dataset_manifest()["objects"]:
        path = data_dir / item["name"]
        headers, final_url = {}, item["url"]
        cached = path.is_file()
        if cached:
            packed = path.read_bytes()
        else:
            with urllib.request.urlopen(item["url"], timeout=60) as response:
                if response.url != item["url"]:
                    raise ValueError("unexpected dataset redirect")
                final_url = response.url
                headers = {k.lower(): v for k, v in response.headers.items()}
                packed = response.read(item["gzip_bytes"] + 1)
            md5_headers = [x.strip().split("=", 1)[1]
                           for x in headers.get("x-goog-hash", "").split(",")
                           if x.strip().startswith("md5=")]
            if len(md5_headers) != 1 or base64.b64decode(md5_headers[0]) != hashlib.md5(packed).digest():
                raise ValueError("Google object checksum missing or mismatched")
        raw = validate_object(packed, item)
        if not cached:
            path.write_bytes(packed)
        (data_dir / item["name"][:-3]).write_bytes(raw)
        receipt["objects"].append(dict(name=item["name"], cached=cached,
            url=item["url"], final_url=final_url, gzip_sha256=digest(packed),
            decompressed_sha256=digest(raw), response_headers={k: headers[k] for k in
                ("x-goog-hash", "x-goog-generation", "etag", "last-modified", "content-length") if k in headers}))
    # Do not replace an earlier receipt with a cache-only observation.
    out = data_dir / "verification-receipt.json"
    out.write_text(json.dumps(receipt, indent=2) + "\n")
    return receipt


def verify_data(data_dir):
    manifest = dataset_manifest()
    raw = {}
    for item in manifest["objects"]:
        packed = (data_dir / item["name"]).read_bytes()
        body = validate_object(packed, item)
        saved = (data_dir / item["name"][:-3]).read_bytes()
        if body != saved:
            raise ValueError("saved decompressed IDX differs from authenticated gzip")
        raw[item["role"]] = saved
    for split in manifest["splits"]:
        first, rows = split["first"], split["rows"]
        image = raw[split["images_role"]][16 + first * 784:16 + (first + rows) * 784]
        labels = raw[split["labels_role"]][8 + first:8 + first + rows]
        check_digest(image, "sha256", split["image_payload_sha256"])
        check_digest(labels, "sha256", split["label_payload_sha256"])
    return raw


def pool(image):
    if len(image) != 784:
        raise ValueError("one image must have 784 pixels")
    return [f32((image[(2*r)*28+2*c] + image[(2*r)*28+2*c+1] +
                 image[(2*r+1)*28+2*c] + image[(2*r+1)*28+2*c+1]) / 1020)
            for r in range(14) for c in range(14)]


def random_word(seed, counter):
    z = (seed + 0x9e3779b97f4a7c15 * (counter + 1)) & MASK64
    z = ((z ^ (z >> 30)) * 0xbf58476d1ce4e5b9) & MASK64
    z = ((z ^ (z >> 27)) * 0x94d049bb133111eb) & MASK64
    return z ^ (z >> 31)


def epoch_order(rows, seed, epoch):
    order = list(range(rows))
    counter = 0
    for _ in range(epoch + 1):
        for i in range(rows - 1, 0, -1):
            j = random_word(seed, counter) % (i + 1)
            order[i], order[j] = order[j], order[i]
            counter += 1
    return order


def initial_value(seed, counter, width):
    unit = (random_word(seed, counter) >> 11) / 9007199254740992.0
    bound = f32(1 / math.sqrt(width))
    return f32(float(bound) * (2 * unit - 1))


def initial_parameters():
    # Counter positions include both data entries, as specified before learning.
    return ([initial_value(11801, k, 196) for k in range(206, 3358)] +
            [initial_value(11801, k, 16) for k in range(3358, 3528)])


def mlp(x, target, parameters, width, hidden, classes):
    """Closed-form two-affine-layer gradient; not an optimizer or training loop.

    Forward products, running sums and stored intermediates round to f32,
    matching the declared tensor IR profile (not eager compensated dots).
    Reverse values accumulate in f64; published gradients round to f32.
    """
    end_w1 = width * hidden
    end_b1 = end_w1 + hidden
    end_w2 = end_b1 + hidden * classes
    if len(x) != width or len(target) != classes or len(parameters) != end_w2 + classes:
        raise ValueError("MLP shape mismatch")
    w1, b1 = parameters[:end_w1], parameters[end_w1:end_b1]
    w2, b2 = parameters[end_b1:end_w2], parameters[end_w2:]
    pre = [f32(sum_f32(x[i] * w1[i*hidden+j] for i in range(width)) + b1[j])
           for j in range(hidden)]
    h = [v if v > 0 else 0.0 for v in pre]
    y = [f32(sum_f32(h[j] * w2[j*classes+k] for j in range(hidden)) + b2[k])
         for k in range(classes)]
    d = [f32(y[k] - target[k]) for k in range(classes)]
    loss = sum_f32(v*v for v in d)
    dy = [2 * float(v) for v in d]
    dh = []
    for j in range(hidden):
        # Reverse matmul visits outputs in order; its f64 additions are explicit.
        value = 0.0
        for k in range(classes):
            value += dy[k] * w2[j*classes+k]
        dh.append(value if pre[j] > 0 else 0.0)
    g = ([f32(x[i] * dh[j]) for i in range(width) for j in range(hidden)] +
         [f32(v) for v in dh] +
         [f32(h[j] * dy[k]) for j in range(hidden) for k in range(classes)] +
         [f32(v) for v in dy])
    return y, loss, g, pre


def tiny_oracle():
    x = [0.5, -0.25, 0.75, 1.0]
    w1 = [0.25, -0.5, 0.125, -0.25, 0.25, 0.5,
          0.5, 0.125, -0.25, 0.125, -0.125, 0.25]
    b1 = [0.0625, -0.0625, 0.125]
    w2 = [0.5, -0.25, -0.125, 0.5, 0.25, 0.125]
    b2, target = [-0.0625, 0.03125], [0.0, 1.0]
    y, loss, g, pre = mlp(x, target, w1+b1+w2+b2, 4, 3, 2)
    # A second calculation uses exact rationals and symbolic chain rules.
    q = Fraction
    qpre = [sum(q(x[i])*q(w1[i*3+j]) for i in range(4)) + q(b1[j]) for j in range(3)]
    h = [max(z, q(0)) for z in qpre]
    qy = [sum(h[j]*q(w2[j*2+k]) for j in range(3)) + q(b2[k]) for k in range(2)]
    delta = [2*(qy[k]-q(target[k])) for k in range(2)]
    dh = [sum(delta[k]*q(w2[j*2+k]) for k in range(2)) if qpre[j] > 0 else q(0) for j in range(3)]
    qg = ([q(v)*dh[j] for v in x for j in range(3)] + dh +
          [h[j]*delta[k] for j in range(3) for k in range(2)] + delta)
    exact = (y == [float(z) for z in qy] and g == [float(z) for z in qg] and
             loss == float(sum((qy[k]-q(target[k]))**2 for k in range(2))))
    if not exact:
        raise ValueError("dyadic gradient differs from exact rational chain rule")
    return dict(shape=[4,3,2], x=x, target=target, parameters=w1+b1+w2+b2,
                preactivation=pre, logits=y, sum_squared_error=loss,
                gradient=g, fc1_weight_gradient=g[:12], fc1_bias_gradient=g[12:15],
                exact_fraction_check=exact)


def f32_sha(values):
    return digest(b"".join(struct.pack("<f", x) for x in values))


def oracle(data_dir):
    raw = verify_data(data_dir)
    p = initial_parameters()
    order = epoch_order(4096, 11802, 0)
    rows = []
    for row in (0, order[0]):
        image = raw["train_images"][16+row*784:16+(row+1)*784]
        label = raw["train_labels"][8+row]
        x = pool(image)
        target = [float(i == label) for i in range(10)]
        y, loss, g, pre = mlp(x, target, p, 196, 16, 10)
        probes = [0, 1, 15, 16, 31, 127, 255, 511, 1023, 2047, 3000, 3135,
                  3136, 3151, 3152, 3200, 3311, 3312, 3321]
        rows.append(dict(source_index=row, label=label, x_sha256=f32_sha(x),
            logits=y, sum_squared_error=loss, preactivation=pre,
            gradient_f32_sha256=f32_sha(g), fc1_gradient_f32_sha256=f32_sha(g[:3136]),
            gradient_probes={str(i):g[i] for i in probes}))
    result = dict(schema=1, scope="initialization and fixed forward/gradient only; no optimization",
        arithmetic="binary32 products, sequential binary32 running sums and stored intermediates; f64 reverse accumulation, f32 published gradients",
        tolerances=TOLERANCES,
        tiny=tiny_oracle(), parameters_f32_sha256=f32_sha(p),
        parameter_probes={str(i):p[i] for i in (0, 3135, 3136, 3151, 3152, 3311, 3312, 3321)},
        epoch0_order_sha256=digest(b"".join(struct.pack("<q", v) for v in order)),
        epoch0_first16=order[:16], rows=rows)
    return result


def c_header(data_dir):
    raw = verify_data(data_dir)
    p = initial_parameters()
    row = 0  # Original official TRAIN row0, independent of training shuffle.
    label = raw["train_labels"][8+row]
    x = pool(raw["train_images"][16+row*784:16+(row+1)*784])
    y, loss, gradient, _ = mlp(x, [float(i == label) for i in range(10)], p, 196, 16, 10)
    lines = ["/* Independent fixed MNIST reference; generated before optimization. */",
             f"static const int mnist_reference_row = {row};",
             f"static const int mnist_reference_label = {label};",
             f"static const float mnist_reference_loss = {loss.hex()}f;"]
    for name, values in (("input",x),("parameters",p),("logits",y),("gradient",gradient)):
        lines.append(f"static const float mnist_reference_{name}[{len(values)}] = {{" +
                     ",".join(float(v).hex()+"f" for v in values) + "};")
    return "\n".join(lines)+"\n"


def read_floats(path, count, code):
    raw = path.read_bytes()
    size = struct.calcsize(code)
    if len(raw) != count * size:
        raise ValueError(f"{path.name}: expected {count} values/{count*size} bytes, got {len(raw)} bytes")
    values = list(struct.unpack(f"<{count}{code}", raw))
    if not all(math.isfinite(v) for v in values):
        raise ValueError(f"{path.name}: non-finite captured value")
    return values


def compare_values(name, actual, expected, code):
    if len(actual) != len(expected):
        raise ValueError(f"{name}: count mismatch")
    rule = TOLERANCES[name]
    maximum = 0.0
    for i, (a, e) in enumerate(zip(actual, expected)):
        if not math.isfinite(a) or not math.isfinite(e):
            raise ValueError(f"{name}[{i}]: non-finite value")
        error = abs(a-e)
        maximum = max(maximum, error)
        if "comparison" in rule:
            ok = struct.pack("<"+code, a) == struct.pack("<"+code, e)
        else:
            ok = error <= rule["absolute"] + rule["relative"] * abs(e)
        if not ok:
            raise ValueError(f"{name}[{i}]: got {a!r}, expected {e!r}; predeclared tolerance {rule}")
    return dict(count=len(expected), max_absolute_error=maximum, rule=rule)


def verify_capture(capture_dir, data_dir):
    # Fixed original TRAIN row0. No optimizer step and no test labels used.
    metadata = json.loads((capture_dir / "oracle.json").read_text())
    status = metadata.get("status")
    if not ((type(status) is int and status == 0) or status == "ok"):
        raise ValueError("native oracle status is not ok")
    for key, expected in (("parameters",3322),("inputs",3529)):
        if type(metadata.get(key)) is not int or metadata[key] != expected:
            raise ValueError(f"native oracle {key} count mismatch")
    for key in ("values", "adjoints", "flags"):
        if type(metadata.get(key)) is not int or metadata[key] < 1:
            raise ValueError(f"native oracle {key} count is not a positive integer")
    loss = metadata.get("loss")
    if type(loss) not in (int, float) or not math.isfinite(loss):
        raise ValueError("native oracle loss is not finite")
    raw = verify_data(data_dir)
    x = pool(raw["train_images"][16:16+784])
    label = raw["train_labels"][8]
    p = initial_parameters()
    y, want_loss, gradient, _ = mlp(x, [float(k==label) for k in range(10)], p, 196, 16, 10)
    checks = {}
    for name, filename, want, code in (
            ("parameters","params.f64",p,"d"),
            ("gradient","gradient.f64",gradient,"d"),
            ("scores","scores.f32",y,"f"),
            ("features","features.f32",x,"f")):
        got = read_floats(capture_dir / filename, len(want), code)
        checks[name] = compare_values(name, got, want, code)
    checks["loss"] = compare_values("loss", [loss], [want_loss], "f")
    return dict(verified=True, source="original official TRAIN row0", label=label,
                checks=checks, observed_native_counts={k:metadata[k] for k in
                    ("parameters","inputs","values","adjoints","flags")},
                dataset_manifest_sha256=digest((FIXTURES/"dataset.json").read_bytes()),
                scope="initial public Coax forward and full published gradient; no training-quality or optimizer claim")


def tagged_sha256(tag, payload):
    h = hashlib.sha256(tag.encode()).digest()
    return hashlib.sha256(h+h+payload).digest()


def checkpoint_identities():
    """Independently serialize the frozen public trainer configuration."""
    bits = lambda x: struct.unpack("<Q", struct.pack("<d",x))[0]
    words = ([1,0,0,0] + [bits(x) for x in (0.003,0.0,0.9,0.999,1e-8,0.0)] +
             [0]*9 + [bits(1.0),16,bits(16.0),4096,1,12,1,11802,0,0,0,0])
    if len(words) != 31:
        raise ValueError("frozen trainer identity must contain 31 words")
    optimizer = tagged_sha256("psionic.trainer-config.v1",struct.pack("<31Q",*words))
    prep = b"mnist-idx-pool-v1:28x28;2x2-average;sum/1020;f32;first4096;next1024;official-test-first2048"
    objects = dataset_manifest()["objects"]
    data = b"".join(bytes.fromhex(x["decompressed_sha256"]) for x in objects)
    manifest=hashlib.sha256((FIXTURES/"experiment.json").read_bytes()).digest()
    initial=struct.pack("<3322d",*initial_parameters())
    model=tagged_sha256("psionic.mnist-initial-model.v1",manifest+initial)
    return (model,
            hashlib.sha256(prep).digest(),hashlib.sha256(data).digest(),optimizer)


def parse_checkpoint(raw, identities, revision):
    n = 3322
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
    expected = (revision,11802,revision//256,(revision%256)*16,0,revision,0)
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


def check_pointer(raw, checkpoint):
    if len(raw)!=80 or raw[:8]!=b"PSCP\x01\x00\x00\x00":
        raise ValueError("checkpoint pointer extent/header mismatch")
    if tagged_sha256("psionic.checkpoint-pointer.v1",raw[:48]) != raw[48:]:
        raise ValueError("checkpoint pointer tagged digest mismatch")
    if (struct.unpack_from("<Q",raw,8)[0] != checkpoint["revision"] or
            raw[16:48] != checkpoint["digest"]):
        raise ValueError("stale checkpoint pointer")


def unique_json(pairs):
    out = {}
    for key,value in pairs:
        if key in out:
            raise ValueError("duplicate JSON field")
        out[key]=value
    return out


def check_export(raw, master):
    if len(raw)<8:
        raise ValueError("short safetensors file")
    count = struct.unpack_from("<Q",raw)[0]
    if count<2 or count>16384 or 8+count>len(raw):
        raise ValueError("safetensors header extent mismatch")
    header = json.loads(raw[8:8+count],object_pairs_hook=unique_json)
    shapes = {"fc1.weight":([196,16],0,3136),"fc1.bias":([16],3136,16),
              "fc2.weight":([16,10],3152,160),"fc2.bias":([10],3312,10)}
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
    return dict(tensors=4,words=3322,bytes=len(raw),all_exported_words_match=True)


def score_metrics(scores,labels,baseline):
    if len(scores)!=10*len(labels) or not all(math.isfinite(x) for x in scores):
        raise ValueError("prediction count or finite-value mismatch")
    correct=0;mean=0.0
    for row,label in enumerate(labels):
        values=scores[row*10:(row+1)*10]
        predicted=max(range(10),key=lambda k:values[k])  # first tie wins
        correct+=predicted==label
        mean+=sum_f32(f32(values[k]-float(k==label))**2 for k in range(10))/len(labels)
    return dict(correct=correct,baseline_correct=sum(x==baseline for x in labels),mean_sse=mean)


def verify_run(run_dir,data_dir):
    result=json.loads((run_dir/"result.json").read_text(),object_pairs_hook=unique_json)
    if type(result.get("status"))is not int:
        raise ValueError("result status is not an integer")
    for key,want in (("steps",3072),("train_rows",49152),("test_rows",2048),("checkpoint_bytes",80068)):
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
    expected=struct.pack("<4096q",*epoch_order(4096,11802,6))
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
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("acquire", "verify-data", "oracle", "verify-capture", "verify-run"))
    parser.add_argument("capture_dir", nargs="?", type=Path)
    parser.add_argument("--data-dir", type=Path, default=DEFAULT_DATA)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--header", type=Path)
    args = parser.parse_args()
    if args.command == "acquire":
        result = acquire(args.data_dir)
    elif args.command == "verify-data":
        raw = verify_data(args.data_dir)
        result = dict(verified=True, objects=4, splits=3, payload_bytes=sum(map(len,raw.values())))
    elif args.command == "oracle":
        result = oracle(args.data_dir)
        if args.header:
            args.header.write_text(c_header(args.data_dir))
    elif args.command == "verify-capture":
        if args.capture_dir is None:
            parser.error("verify-capture requires its capture directory")
        result = verify_capture(args.capture_dir, args.data_dir)
    else:
        if args.capture_dir is None:
            parser.error("verify-run requires its run directory")
        result = verify_run(args.capture_dir,args.data_dir)
    text = json.dumps(result, indent=2, allow_nan=False)+"\n"
    if args.out:
        args.out.write_text(text)
    else:
        print(text, end="")
    if args.command=="verify-run" and not result["passed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
