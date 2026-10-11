"""Summarize completed receipts and small saved spans; no model framework import."""
from array import array
import hashlib
import json
import math
from pathlib import Path
import struct
import sys

HERE = Path(__file__).resolve().parent


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def tensor(path, expected):
    with path.open("rb") as stream:
        length = struct.unpack("<Q", stream.read(8))[0]
        assert length < 1024**2
        meta = json.loads(stream.read(length))["hidden"]
        assert meta["dtype"] == "F32" and meta["shape"] == [1, 359, 5120]
        start, end = meta["data_offsets"]
        assert end - start == 359 * 5120 * 4
        stream.seek(8 + length + start)
        raw = stream.read(end - start)
    assert sha(raw) == expected
    return raw


def span(raw, bounds):
    assert len(raw) == 359 * 5120 * 4
    value = array("f")
    value.frombytes(raw[bounds[0] * 5120 * 4 : bounds[1] * 5120 * 4])
    if sys.byteorder != "little":
        value.byteswap()
    assert len(value) == (bounds[1] - bounds[0]) * 5120 and all(math.isfinite(v) for v in value)
    return value


def errors(left, right):
    assert len(left) == len(right) and len(left) > 0 and len(left) % 5120 == 0
    e2 = math.fsum((float(a) - float(b)) ** 2 for a, b in zip(left, right))
    n2 = math.fsum(float(b) ** 2 for b in right)
    cosines = []
    for first in range(0, len(left), 5120):
        a, b = left[first : first + 5120], right[first : first + 5120]
        norms = math.sqrt(math.fsum(float(x) ** 2 for x in a) * math.fsum(float(x) ** 2 for x in b))
        cosines.append(math.fsum(float(x) * float(y) for x, y in zip(a, b)) / norms if norms else (1.0 if a == b else -1.0))
    return {"values": len(left), "max_abs_error": max(abs(float(a) - float(b)) for a, b in zip(left, right)),
            "relative_l2": math.sqrt(e2 / n2) if n2 else (0.0 if e2 == 0 else None),
            "minimum_token_cosine": min(cosines), "zero_reference_norm": not bool(n2)}


def probabilities(logits):
    values = [math.exp(x - max(logits)) for x in logits]
    total = math.fsum(values)
    return [x / total for x in values]


def main():
    output = HERE / "precision-2-summary-complete.json"
    assert not output.exists()
    run = HERE / "precision-2"
    guard = json.loads((run / "guard.json").read_text())
    raw_receipt = (run / "results/receipt.json").read_bytes()
    result = json.loads(raw_receipt)
    assert guard["status"] == "complete" and guard["memory"]["owned_group_stopped"]
    assert guard["result_sha256"] == sha(raw_receipt) and len(result["layers"]) == 64
    cpu = json.loads((HERE / "cpu-1/results/receipt.json").read_text())
    native = json.loads((HERE / "native-1/results/receipt.json").read_text())
    bname, cname = "B_projection_half", "C_projection_and_attention_half"
    b, c = result["heads"][bname], result["heads"][cname]
    arrays = {"A": tensor(HERE / "cpu-1/results/final-hidden.safetensors", cpu["hidden_sha256"]),
              "B": tensor(run / "results" / b["head_inputs_file"], b["hidden_sha256"]),
              "C": tensor(run / "results" / c["head_inputs_file"], c["hidden_sha256"])}
    nraw = (HERE / "native-1/results/rows/result_norm.f32").read_bytes()
    assert sha(nraw) == next(v["sha256"] for v in native["files"] if v["name"] == "result_norm.f32")
    arrays["native"] = nraw
    q = result["record"]["questions"][1]
    assert q["question_id"] == "日本1" and q["option_ids"] == ["true", "false"] and q["option_spans"][1] == [229, 247]
    final_whole = {"B_minus_A": b["final_vs_A"], "C_minus_A": c["final_vs_A"],
                   "C_minus_B": result["final_C_minus_B"]["hidden"],
                   "B_minus_native": b["final_vs_native"], "C_minus_native": c["final_vs_native"]}
    summary = {"schema": "clef.text029-full-precision-summary.v2", "source_sha256": sha(Path(__file__).read_bytes()),
               "receipt_sha256": sha(raw_receipt), "guard_sha256": sha((run / "guard.json").read_bytes()),
               "method": "Existing whole-memory metrics copied verbatim; final question/true-option/false-option metrics from checked saved f32 arrays using Python-f64 math.fsum. No model, Torch, NumPy, GPU or weights loaded.",
               "bounds": guard["bounds"], "seconds": guard["seconds"], "memory": guard["memory"],
               "final_normalized_memory": {}, "questions": [], "layer63_pre_norm": result["layers"][-1]["comparisons"],
               "acceptance": "Original independent-f32 40/104 gate remains unchanged and unmet. These controls are diagnostic only."}
    for label, whole in final_whole.items():
        left, right = label.split("_minus_")
        summary["final_normalized_memory"][label] = {"whole": whole, **{name: errors(span(arrays[left], bounds), span(arrays[right], bounds)) for name, bounds in (("question", q["question_span"]), ("true_option", q["option_spans"][0]), ("false_option", q["option_spans"][1]))}}
    for index, question in enumerate(result["record"]["questions"]):
        assert question["option_ids"] == ["true", "false"]
        values = {"A": result["saved_A_logits"][index], "B": b["logits"][index], "C": c["logits"][index],
                  "native_staged": result["native_logits"]["staged"][index],
                  "native_fused_fields": result["native_logits"]["fused-fields"][index]}
        summary["questions"].append({"id": question["question_id"], "option_ids": question["option_ids"],
            "variants": {name: {"logits": logits, "probabilities": probabilities(logits),
                "top": question["option_ids"][max(range(len(logits)), key=lambda i: logits[i])]} for name, logits in values.items()}})
    output.write_text(json.dumps(summary, indent=2, ensure_ascii=False, allow_nan=False) + "\n")
    print(json.dumps({"file": str(output), "sha256": sha(output.read_bytes()),
        "questions": summary["questions"], "final_normalized_memory": summary["final_normalized_memory"]}, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
