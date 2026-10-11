#!/usr/bin/env python3
"""Compare saved 27B head inputs without loading or executing a backbone.

Run through guard_head_isolation.py. This diagnostic uses only the official
256 MB head on CPU; it does not establish model acceptance or repair captures.
"""
import argparse
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import sys
import time

GIB = 1024**3
CASES = ("text-014", "text-034")
EXPECTED = {
    "joint_schema_model.py": "0e304cf7c6500e8bb59bef7e2afd2c6373f82596dfb3b57d1aa93c175e2dc3a3",
    "joint_head.safetensors": "a010ac04f078e699988e4049cbea5e62c962393f59fec366640b64e8d69a4953",
    "joint_head_config.json": "890be585d75b981201eb96a35f98a9967afe37bc8d220cfdea2e72d56507534f",
}
TENSOR_KEYS = {"attention_mask", "input_ids", "lexical_token_ids", "remapped_input_ids", "hidden", "lexical"}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024**2), b""):
            digest.update(block)
    return digest.hexdigest()


def read_json(path, maximum=128 * 1024**2):
    require(0 < path.stat().st_size <= maximum, f"JSON size is outside the bound: {path}")
    return json.loads(path.read_text())


def write(path, value):
    temporary = path.with_suffix(path.suffix + ".partial")
    temporary.write_text(json.dumps(value, indent=2, allow_nan=False) + "\n")
    temporary.replace(path)


def probabilities(values):
    maximum = max(values)
    weights = [math.exp(value - maximum) for value in values]
    total = sum(weights)
    return [weight / total for weight in weights]


def logits(values, questions):
    require(isinstance(values, list) and len(values) == len(questions), "logit question count differs")
    for row, question in zip(values, questions, strict=True):
        require(len(row) == len(question["option_ids"]) and all(math.isfinite(x) for x in row),
                "logit count or finiteness check failed")
    return values


def compare_logits(candidate, reference, questions):
    logits(candidate, questions)
    logits(reference, questions)
    rows = []
    for left, right, question in zip(candidate, reference, questions, strict=True):
        lp, rp = probabilities(left), probabilities(right)
        li, ri = max(range(len(lp)), key=lp.__getitem__), max(range(len(rp)), key=rp.__getitem__)
        rows.append({"id": question["id"], "type": question["type"], "option_ids": question["option_ids"],
                     "candidate_logits": left, "reference_logits": right,
                     "candidate_probabilities": lp, "reference_probabilities": rp,
                     "candidate_top": question["option_ids"][li], "reference_top": question["option_ids"][ri],
                     "top_agreement": li == ri,
                     "max_abs_dlogit": max(abs(a-b) for a, b in zip(left, right, strict=True)),
                     "max_abs_dp": max(abs(a-b) for a, b in zip(lp, rp, strict=True))})
    return {"compared_values": sum(len(row) for row in candidate), "finite": True,
            "max_abs_dlogit": max(row["max_abs_dlogit"] for row in rows),
            "max_abs_dp": max(row["max_abs_dp"] for row in rows),
            "all_tops_agree": all(row["top_agreement"] for row in rows), "questions": rows}


def normalized_questions(record, count):
    result = [{"id": q["question_id"], "type": q["question_type"], "question_span": q["question_span"],
               "option_spans": q["option_spans"], "option_ids": q["option_ids"]} for q in record["questions"]]
    require(1 <= len(result) <= 4 and len({q["id"] for q in result}) == len(result), "invalid question count or IDs")
    for q in result:
        require(q["type"] in (0, 1, 2), "unsupported question type")
        require(1 <= len(q["option_ids"]) <= 16 and len(q["option_ids"]) == len(q["option_spans"]), "invalid options")
        require(len(set(q["option_ids"])) == len(q["option_ids"]), "duplicate option IDs")
        for span in [q["question_span"], *q["option_spans"]]:
            require(len(span) == 2 and all(type(x) is int for x in span) and 0 <= span[0] < span[1] <= count,
                    "invalid span")
    return result


def row_errors(torch, candidate, reference, identifiers):
    require(candidate.shape == reference.shape and candidate.ndim == 2 and candidate.shape[0] == len(identifiers),
            "row comparison shape differs")
    require(bool(torch.isfinite(candidate).all()) and bool(torch.isfinite(reference).all()), "nonfinite input row")
    row_results, abs_sum, square_sum, reference_square = [], 0.0, 0.0, 0.0
    for first in range(0, len(identifiers), 64):
        left, right = candidate[first:first+64].double(), reference[first:first+64].double()
        delta = left-right
        ab = delta.abs()
        errors = (delta*delta).sum(dim=1)
        ln, rn = (left*left).sum(dim=1), (right*right).sum(dim=1)
        dots = (left*right).sum(dim=1)
        abs_sum += float(ab.sum())
        square_sum += float(errors.sum())
        reference_square += float(rn.sum())
        for offset in range(left.shape[0]):
            lnorm, rnorm = math.sqrt(float(ln[offset])), math.sqrt(float(rn[offset]))
            cosine = float(dots[offset])/(lnorm*rnorm) if lnorm and rnorm else None
            row_results.append({"row": first+offset, "id": identifiers[first+offset],
                                "max_abs_error": float(ab[offset].max()), "rmse": math.sqrt(float(errors[offset])/left.shape[1]),
                                "candidate_l2": lnorm, "reference_l2": rnorm, "cosine": cosine})
    cosines = [r["cosine"] for r in row_results if r["cosine"] is not None]
    return {"shape": list(candidate.shape), "compared_values": candidate.numel(), "finite": True,
            "max_abs_error": max(r["max_abs_error"] for r in row_results), "mean_abs_error": abs_sum/candidate.numel(),
            "rmse": math.sqrt(square_sum/candidate.numel()),
            "relative_l2": math.sqrt(square_sum/reference_square) if reference_square else None,
            "minimum_row_cosine": min(cosines) if cosines else None,
            "mean_row_cosine": sum(cosines)/len(cosines) if cosines else None,
            "undefined_cosine_rows": len(row_results)-len(cosines), "last_row": row_results[-1],
            "worst_absolute_rows": sorted(row_results, key=lambda r: -r["max_abs_error"])[:20],
            "worst_cosine_rows": sorted((r for r in row_results if r["cosine"] is not None), key=lambda r: r["cosine"])[:20]}


def load_case(torch, load_file, case, native_dir, args, capture, config):
    metadata_path = args.reference_dir / (case + ".json")
    metadata = read_json(metadata_path)
    require(metadata == next(row for row in capture["cases"] if row["id"] == case), "case differs from complete reference")
    checks = metadata["immutable_checks"]
    require(checks["all_exact"] and checks["mps_hidden_unchanged_after_head"] and checks["lexical_reread_exact"], "reference immutable checks failed")
    require(checks["before_trunk"] == checks["after_trunk"] == checks["after_head"] and checks["cpu_head_before"] == checks["cpu_head_after"], "reference snapshot checks differ")
    info = metadata["head_inputs"]
    require(info["file"] == case + "-head-inputs.safetensors", "unexpected capture filename")
    path = args.reference_dir / info["file"]
    require(path.stat().st_size == info["bytes"] <= 32*1024**2 and sha(path) == info["sha256"], "captured tensors changed")
    tensors = load_file(str(path), device="cpu")
    require(set(tensors) == TENSOR_KEYS, "capture tensor names differ")
    ids = metadata["input_ids"]
    require(1 <= len(ids) <= 2048 and all(type(t) is int and t >= 0 for t in ids), "input IDs or bound invalid")
    require(tensors["input_ids"].dtype == torch.int64 and tensors["input_ids"].tolist() == [ids], "captured IDs differ")
    require(len(metadata["records"]) == 1 and metadata["records"][0]["input_ids"] == ids and metadata["records"][0].get("media") is None, "captured record differs")
    questions = normalized_questions(metadata["records"][0], len(ids))
    tokens = sorted({token for q in questions for start, end in q["option_spans"] for token in ids[start:end]})
    remap = {token:index for index, token in enumerate(tokens)}
    head_ids = [remap.get(token, 0) for token in ids]
    require(tensors["lexical_token_ids"].dtype == torch.int64 and tensors["lexical_token_ids"].tolist() == tokens, "option lexical coverage differs")
    require(tensors["remapped_input_ids"].dtype == torch.int64 and tensors["remapped_input_ids"].tolist() == [head_ids], "compact IDs differ")
    require(tensors["attention_mask"].dtype == torch.int64 and tensors["attention_mask"].tolist() == [[1]*len(ids)], "mask differs")
    require(tensors["hidden"].dtype == torch.bfloat16 and list(tensors["hidden"].shape) == [1,len(ids),5120], "hidden dtype/shape differs")
    require(tensors["lexical"].dtype == torch.bfloat16 and list(tensors["lexical"].shape) == [len(tokens),5120], "lexical dtype/shape differs")
    def tensor_sha(value):
        return hashlib.sha256(value.contiguous().view(torch.uint8).numpy().tobytes()).hexdigest()
    require(tensor_sha(tensors["hidden"]) == checks["original_bf16_hidden_sha256"] and tensor_sha(tensors["lexical"]) == checks["original_bf16_lexical_sha256"], "original BF16 input bytes differ")
    for name in ("hidden", "lexical", "attention_mask", "input_ids"):
        tensor = tensors["remapped_input_ids"] if name == "input_ids" else tensors[name]
        actual = tensor.float() if name in ("hidden","lexical") else tensor
        require(tensor_sha(actual) == checks["cpu_head_before"][name]["sha256"], "CPU f32 head inputs differ")
    native_json, native_hidden = native_dir / "dump.json", native_dir / "hidden.f32"
    dump = read_json(native_json)
    require(dump["hidden_size"] == 5120 and dump["head_config"] == config, "native head config differs")
    require(dump["input_ids"] == ids and dump["questions"] == questions, "native IDs/questions/spans/options differ")
    require(set(dump["lexical"]) == {str(t) for t in tokens} and all(len(dump["lexical"][str(t)]) == 5120 for t in tokens), "native lexical coverage/width differs")
    require(native_hidden.stat().st_size == len(ids)*5120*4 and sys.byteorder == "little", "native hidden byte format differs")
    nh = torch.frombuffer(bytearray(native_hidden.read_bytes()), dtype=torch.float32).reshape(len(ids),5120)
    nl = torch.tensor([dump["lexical"][str(t)] for t in tokens], dtype=torch.float32)
    rh, rl = tensors["hidden"][0].float(), tensors["lexical"].float()
    for name in ("hf_head_logits", "gguf_head_logits"):
        logits(dump[name], questions)
    logits(metadata["logits"], questions)
    metadata = dict(metadata, bf16_logits=metadata["logits"])
    evidence = {"case_metadata_sha256":sha(metadata_path), "reference_tensor_sha256":info["sha256"],
        "native_dump_sha256":sha(native_json), "native_hidden_sha256":sha(native_hidden),
        "source_request_sha256":metadata["source_request_sha256"], "reference_immutable_checks":checks,
        "exact_input_ids":True, "exact_questions_spans_option_ids":True, "exact_lexical_token_coverage":True,
        "before_mask_exact_ones":True, "input_tokens":len(ids), "hidden_size":5120, "lexical_tokens":len(tokens)}
    return metadata,dump,questions,ids,tokens,head_ids,nh,nl,rh,rl,evidence


def evaluate_case(torch, official, head, load_file, case, native_dir, args, capture, config, prior_control):
    began = time.monotonic()
    metadata, dump, questions, ids, option_ids, head_ids, nh, nl, rh, rl, evidence = load_case(
        torch, load_file, case, native_dir, args, capture, config)
    result = {"id": case, "evidence": evidence, "questions": questions,
              "hidden_error": row_errors(torch, nh, rh, ids), "lexical_error": row_errors(torch, nl, rl, option_ids)}
    encoded = official.EncodedRecord(input_ids=tuple(head_ids), record_id="diagnostic", questions=tuple(
        official.EncodedQuestion(q["id"], q["type"], tuple(q["question_span"]), tuple(tuple(s) for s in q["option_spans"]), tuple(q["option_ids"]))
        for q in questions))
    ids_tensor, mask = torch.tensor([head_ids], dtype=torch.long), torch.ones(1, len(ids), dtype=torch.long)
    with torch.inference_mode():
        nn, rn = head.hidden_norm(nh), head.hidden_norm(rh)
        result["normalized_hidden_error"] = row_errors(torch, nn, rn, ids)
        result["pooled_hidden_error"] = {}
        for label, left, right in [("raw", nh, rh), ("normalized", nn, rn)]:
            spans = [("final_token", (len(ids)-1, len(ids)))]
            for q in questions:
                spans.append((f"question:{q['id']}", q["question_span"]))
                spans.extend((f"option:{q['id']}:{option}", span) for option, span in zip(q["option_ids"], q["option_spans"], strict=True))
            result["pooled_hidden_error"][label] = row_errors(torch,
                torch.stack([left[a:b].mean(0) for _, (a, b) in spans]),
                torch.stack([right[a:b].mean(0) for _, (a, b) in spans]), [name for name, _ in spans])
        del nn, rn
        outputs = {}
        for name, hidden, lexical in (("reference_hidden_reference_lexical", rh, rl),
                                      ("native_hidden_native_lexical", nh, nl),
                                      ("native_hidden_reference_lexical", nh, rl),
                                      ("reference_hidden_native_lexical", rh, nl)):
            started = time.monotonic()
            hidden_copy, lexical_copy = hidden.clone(), lexical.clone()
            raw = head(hidden.unsqueeze(0), ids_tensor, mask, [encoded], lexical)[0]
            values = logits([row.float().tolist() for row in raw], questions)
            require(torch.equal(hidden, hidden_copy) and torch.equal(lexical, lexical_copy)
                    and ids_tensor.tolist() == [head_ids] and bool((mask == 1).all()), "CPU head changed its inputs")
            outputs[name] = {"logits": values, "seconds": time.monotonic()-started,
                             "against_saved_reference_cpu_f32": compare_logits(values, metadata["bf16_logits"], questions)}
            del hidden_copy, lexical_copy, raw
    control = outputs["reference_hidden_reference_lexical"]["logits"]
    for output in outputs.values():
        output["against_reference_f32_control"] = compare_logits(output["logits"], control, questions)
    result["variants"] = outputs
    result["control_repeat"] = compare_logits(control, prior_control["f32_logits"], questions)
    actual = outputs["native_hidden_native_lexical"]["logits"]
    result["rust_hf_head_same_inputs"] = compare_logits(dump["hf_head_logits"], actual, questions)
    result["rust_hf_head_same_inputs"].update(max_abs_dlogit_bound=1e-3,
        within_bound=result["rust_hf_head_same_inputs"]["max_abs_dlogit"] <= 1e-3)
    result["rust_gguf_head_same_inputs"] = compare_logits(dump["gguf_head_logits"], actual, questions)
    result["seconds"] = time.monotonic()-began
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("checkpoint", "reference-dir", "native-014", "native-034", "output-dir"):
        parser.add_argument("--"+name, type=Path, required=True)
    args = parser.parse_args()
    resources = set(os.environ.get("OPENAGENTS_LEASES", "").split(","))
    require("memory" in resources and not resources.intersection({"quiet", "gpu"}), "requires memory lease only; no GPU or quiet lease")
    require(os.environ.get("CLEF_HEAD_ISOLATION_GUARDED") == "1", "run through the owned-process guard")
    require(not args.output_dir.exists(), "output directory already exists; preserve earlier evidence")
    args.output_dir.mkdir(parents=True)
    report = {"kind": "native27b_head_input_isolation", "status": "validating", "diagnostic_only": True,
              "device": "cpu", "dtype": "float32", "threads": 2, "cases": [], "script_sha256": sha(Path(__file__)),
              "conditions": "Shared machine; saved inputs and official head only. No trunk load, GPU work, or model-acceptance claim.",
              "interpretation": "Cross-swaps separate input effects under one head; nonlinear effects are not additive.",
              "reference_execution": "BF16 MPS trunk with official CPU f32 head; clean immutable captures"}
    destination = args.output_dir / "isolation.json"
    write(destination, report)
    try:
        capture = read_json(args.reference_dir / "reference.json")
        report.update(reference_capture_status=capture["status"], reference_capture_error=capture.get("error"),
                      reference_capture_sha256=sha(args.reference_dir / "reference.json"))
        require(capture["status"] == "complete" and capture["completed"] and capture["all_immutable_checks_passed"]
                and len(capture["cases"]) == 40 and capture["head_device"] == "cpu" and capture["head_dtype"] == "float32",
                "clean reference is incomplete or has the wrong execution configuration")
        capture["source_sha256"] = capture["reference_source_sha256"]
        for name, key in (("joint_head.safetensors", "head_sha256"), ("joint_head_config.json", "head_config_sha256"),
                          ("joint_schema_model.py", "source_sha256")):
            path = args.checkpoint / name
            require(path.stat().st_size <= 300*1024**2 and sha(path) == EXPECTED[name] == capture[key], "official source/head/config differs")
        config = read_json(args.checkpoint / "joint_head_config.json")
        require(config == {"hidden_size": 5120, "width": 1024, "routing_layers": 2, "layers": 4, "heads": 16, "feedforward": 4096}, "unexpected head config")
        prior = {row["id"]: {"f32_logits": row["logits"]} for row in capture["cases"] if row["id"] in CASES}
        require(set(prior) == set(CASES), "clean reference cases are missing")
        for name in ("OMP_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "VECLIB_MAXIMUM_THREADS", "RAYON_NUM_THREADS"):
            require(os.environ.get(name) == "2", "CPU thread bound missing")
        import torch
        from safetensors.torch import load_file
        torch.set_num_threads(2)
        torch.set_num_interop_threads(1)
        torch.set_default_device("cpu")
        spec = importlib.util.spec_from_file_location("official_head_isolation", args.checkpoint / "joint_schema_model.py")
        official = importlib.util.module_from_spec(spec)
        sys.modules[spec.name] = official
        spec.loader.exec_module(official)
        head = official.JointSchemaHead(**config)
        state = load_file(str(args.checkpoint / "joint_head.safetensors"), device="cpu")
        head.load_state_dict(state, strict=True)
        del state
        head = head.to(device="cpu", dtype=torch.float32).eval()
        report.update(status="running", torch=str(torch.__version__), head_config=config,
                      head_sha256=capture["head_sha256"], source_sha256=capture["source_sha256"],
                      prior_cpu_control_sha256=sha(args.reference_dir / "reference.json"))
        write(destination, report)
        for case, native in zip(CASES, (args.native_014, args.native_034), strict=True):
            row = evaluate_case(torch, official, head, load_file, case, native, args, capture, config, prior[case])
            report["cases"].append(row)
            write(args.output_dir / (case + ".json"), row)
            write(destination, report)
            print(json.dumps({"case": case, "status": "complete", "same_input_hf_head_max_abs_dlogit": row["rust_hf_head_same_inputs"]["max_abs_dlogit"]}), flush=True)
        report["status"] = "complete"
    except BaseException as error:
        report.update(status="failed", error_type=type(error).__name__, error=str(error))
        raise
    finally:
        write(destination, report)


if __name__ == "__main__":
    main()
