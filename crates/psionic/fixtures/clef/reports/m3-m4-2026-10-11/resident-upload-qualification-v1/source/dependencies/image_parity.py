#!/usr/bin/env python3
"""Compare native image decisions with Cloudflare's complete reference model.

Run reference and native modes separately so both models need not be resident.
This script never downloads weights or starts a server.
"""

import argparse
import base64
import hashlib
import importlib.util
import importlib.metadata
import json
import math
import os
from pathlib import Path
import sys
import time
import urllib.error
import urllib.request


DEFAULT_CASES = Path(__file__).resolve().parents[1] / "images" / "cases.json"
HTTP = urllib.request.build_opener(urllib.request.ProxyHandler({}))


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_cases(path, model):
    cases = json.loads(path.read_text())
    if not cases or len({case["id"] for case in cases}) != len(cases):
        raise ValueError("cases must be nonempty and have unique IDs")
    rows = []
    for case in cases:
        if not case["images"] or not case["questions"] or set(case["expected"]) != set(case["questions"]):
            raise ValueError(f"incomplete image case: {case['id']}")
        images = [path.parent / name for name in case["images"]]
        request = {"model": model, "state": case["state"], "questions": case["questions"],
                   "images": [base64.b64encode(image.read_bytes()).decode("ascii") for image in images]}
        rows.append((case, request, [sha(image) for image in images]))
    return rows


def load_suite(args):
    if not args.text_suite:
        return load_cases(args.cases, args.model)
    # Keep source number spellings for native HTTP. The official Python call
    # receives json.loads(original bytes), including its float/int semantics.
    from e2e import Raw, dump
    rows = []
    with args.text_suite.open(encoding="utf-8") as source:
        for index, line in enumerate(source):
            raw = json.loads(line, parse_float=Raw, parse_int=Raw)
            raw["model"] = args.model
            body = dump(raw)
            request = json.loads(body)
            if not request.get("questions") or request.get("images") or request.get("videos"):
                raise ValueError(f"invalid text suite request {index}")
            case = {"id": f"text-{index:03d}", "expected": None, "request_body": body,
                    "source_request_sha256": hashlib.sha256(line.encode()).hexdigest()}
            rows.append((case, request, []))
    if not rows:
        raise ValueError("text suite is empty")
    return rows


def suite_sha(args):
    return sha(args.text_suite or args.cases)


def validate_response(response, request):
    if response.get("model") != request["model"] or set(response["answers"]) != set(request["questions"]):
        raise ValueError("response model or question set does not match the request")
    usage = response["usage"]
    if (type(usage["input_tokens"]) is not int or usage["input_tokens"] <= 0
            or type(usage["output_tokens"]) is not int or usage["output_tokens"] != 0):
        raise ValueError("response has invalid token counts")
    if (usage.get("cached_input_tokens", 0) != 0 or response.get("cached", False)
            or response.get("psionic", {}).get("timing", {}).get("cached", False)):
        raise ValueError("image acceptance requires an uncached response")
    for key, question in request["questions"].items():
        answer = response["answers"][key]
        if answer["type"] != question["type"]:
            raise ValueError(f"wrong answer type for {key}")
        if answer["type"] == "noul":
            probabilities = [answer["noul"]]
        else:
            options = (list(question["criteria"]) if answer["type"] == "choice" else
                       [str(i) for i in range(len(question["criteria"]))])
            if set(answer["probabilities"]) != set(options):
                raise ValueError(f"wrong probability options for {key}")
            probabilities = list(answer["probabilities"].values())
            if any(type(value) not in (int, float) or not math.isfinite(value) or not 0 <= value <= 1
                   for value in probabilities):
                raise ValueError(f"invalid probability for {key}")
            if not math.isclose(sum(probabilities), 1.0, abs_tol=len(options) * 0.0001):
                raise ValueError(f"probabilities do not sum to one for {key}")
            if answer["type"] == "choice" and answer["choice"] not in options:
                raise ValueError(f"invalid chosen option for {key}")
            if (type(answer["confidence"]) not in (int, float)
                    or not math.isfinite(answer["confidence"]) or not 0 <= answer["confidence"] <= 1):
                raise ValueError(f"invalid confidence for {key}")
            # The released reference rounds probabilities and answers to four
            # decimal places independently. Preserve that known rounding bound.
            if not math.isclose(answer["confidence"], max(probabilities), abs_tol=0.000100001):
                raise ValueError(f"confidence disagrees with probabilities for {key}")
            if (answer["type"] == "choice"
                    and max(probabilities) - answer["probabilities"][answer["choice"]] > 0.000100001):
                raise ValueError(f"choice is not a most probable option for {key}")
            if answer["type"] == "score" and (
                type(answer["score"]) not in (int, float) or not math.isfinite(answer["score"])
                or not 0 <= answer["score"] <= len(options) - 1
            ):
                raise ValueError(f"invalid score for {key}")
            if answer["type"] == "score":
                expectation = sum(index * answer["probabilities"][option] for index, option in enumerate(options))
                tolerance = 0.00005 * (1 + sum(range(len(options)))) + 1e-9
                if not math.isclose(answer["score"], expectation, abs_tol=tolerance):
                    raise ValueError(f"score disagrees with probabilities for {key}")
        if any(type(value) not in (int, float) or not math.isfinite(value) or not 0 <= value <= 1
               for value in probabilities):
            raise ValueError(f"invalid probability for {key}")


def validate_checkpoint(checkpoint, maximum_gib):
    required = ["joint_schema_model.py", "joint_head.safetensors", "joint_head_config.json",
                "model.safetensors.index.json", "processor_config.json", "tokenizer.json",
                "tokenizer_config.json", "config.json"]
    for name in required:
        if not (checkpoint / name).is_file():
            raise ValueError(f"checkpoint is incomplete: {name}")
    index = json.loads((checkpoint / "model.safetensors.index.json").read_text())
    mapping = index["weight_map"]
    if not mapping:
        raise ValueError("checkpoint index contains no weights")
    total = (checkpoint / "joint_head.safetensors").stat().st_size
    for name in sorted(set(mapping.values())):
        if Path(name).name != name:
            raise ValueError("checkpoint shard must be a direct child of the checkpoint directory")
        shard = checkpoint / name
        with shard.open("rb") as source:
            size = shard.stat().st_size
            header_size = int.from_bytes(source.read(8), "little")
            if not 0 < header_size <= min(size - 8, 16 * 1024 * 1024):
                raise ValueError(f"invalid safetensors header: {name}")
            header = json.loads(source.read(header_size))
        expected = {key for key, value in mapping.items() if value == name}
        if set(header) - {"__metadata__"} != expected:
            raise ValueError(f"checkpoint shard disagrees with the weight index: {name}")
        for key in expected:
            start, end = header[key]["data_offsets"]
            if not 0 <= start < end <= size - 8 - header_size:
                raise ValueError(f"checkpoint shard is incomplete: {name}/{key}")
        total += size
    if total > maximum_gib * 1024 ** 3:
        raise ValueError("checkpoint exceeds the configured model size bound")
    return {"index_sha256": sha(checkpoint / "model.safetensors.index.json"),
            "shards": sorted(set(mapping.values())), "checkpoint_file_bytes": total}


def post(endpoint, request, timeout, raw_body=None):
    headers = {"Content-Type": "application/json"}
    if os.environ.get("OPENAGENTS_API_KEY"):
        headers["Authorization"] = "Bearer " + os.environ["OPENAGENTS_API_KEY"]
    operation = urllib.request.Request(endpoint.rstrip("/") + "/v1/systemone",
                                       (raw_body or json.dumps(request)).encode(), headers=headers)
    try:
        with HTTP.open(operation, timeout=timeout) as response:
            return response.status, json.load(response)
    except urllib.error.HTTPError as error:
        return error.code, json.load(error)


def check_refusals(args, example):
    examples = [
        ("images_not_array", {"images": "aGk="}, "invalid_request"),
        ("image_not_string", {"images": [42]}, "invalid_request"),
        ("invalid_base64", {"images": ["not base64"]}, "invalid_request"),
        ("invalid_image", {"images": ["aGk="]}, "invalid_request"),
        ("remote_url", {"images": ["https://example.com/image.png"]}, "invalid_request"),
        ("wrong_mime", {"images": ["data:image/gif;base64," + example["images"][0]]}, "invalid_request"),
        ("unsupported_gif", {"images": [base64.b64encode(b"GIF89a").decode()]}, "not_admitted"),
        ("too_many_images", {"images": [example["images"][0]] * 5}, "not_admitted"),
        ("video", {"images": [], "videos": ["aGk="]}, "not_admitted"),
        ("processor_override", {"media_kwargs": {"max_pixels": 65536}}, "not_admitted"),
    ]
    results = []
    for name, changes, expected_code in examples:
        request = {**example, **changes}
        status, response = post(args.endpoint, request, args.timeout)
        passed = status == 400 and response.get("error", {}).get("code") == expected_code
        results.append({"id": name, "http_status": status, "response": response, "passed": passed})
    return results


def top_answer(answer):
    if answer["type"] == "noul":
        return "true" if answer["noul"] >= 0.5 else "false"
    if answer["type"] == "choice":
        return answer["choice"]
    return max(answer["probabilities"], key=answer["probabilities"].__getitem__)


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    pending = path.with_suffix(path.suffix + ".pending")
    pending.write_text(json.dumps(value, indent=2, allow_nan=False) + "\n")
    pending.replace(path)


def token_difference(actual, expected):
    if actual is None:
        return {"captured": False, "expected_length": len(expected)}
    for index in range(max(len(actual), len(expected))):
        left = actual[index] if index < len(actual) else None
        right = expected[index] if index < len(expected) else None
        if type(left) is not int or left != right:
            return {"index": index, "actual": left, "expected": right,
                    "actual_length": len(actual), "expected_length": len(expected)}
    return None


def snapshot_input_ids(batch):
    # Copy before model execution and after it. Neither snapshot shares device
    # or CPU tensor storage with the call whose identity it records.
    return batch["input_ids"][0].detach().cpu().clone().tolist()


def snapshot_logits(output):
    return [values.detach().float().cpu().clone().tolist() for values in output[0]]


class ReferenceCapture:
    def __init__(self, ids=snapshot_input_ids, logits=snapshot_logits):
        self.snapshot_ids = ids
        self.snapshot_logits = logits
        self.begin(None, [])

    def begin(self, case_id, expected):
        self.case_id = case_id
        self.expected = list(expected)
        self.record_ids = None
        self.before_ids = None
        self.after_ids = None
        self.logits = None

    def before(self, _model, inputs):
        if self.case_id is None or self.before_ids is not None:
            raise ValueError("reference forward has no active case or ran more than once")
        batch = inputs[0]
        self.before_ids = list(self.snapshot_ids(batch))
        if len(batch["records"]) != 1:
            raise ValueError("reference forward must contain exactly one record")
        self.record_ids = list(batch["records"][0].input_ids)
        usage = {"usage": {"input_tokens": len(self.expected)}}
        validate_input_ids(self.record_ids, self.expected, usage)
        validate_input_ids(self.before_ids, self.expected, usage)

    def after(self, _model, inputs, output):
        if self.before_ids is None or self.after_ids is not None:
            raise ValueError("reference forward capture is missing or duplicated")
        self.after_ids = list(self.snapshot_ids(inputs[0]))
        self.logits = self.snapshot_logits(output)

    def validate(self, response):
        if self.before_ids is None or self.after_ids is None or self.logits is None:
            raise ValueError("reference forward capture is incomplete")
        # Preserve the existing post-forward gate and additionally prove that
        # both the encoded record and actual tensor matched before execution.
        for values in [self.record_ids, self.before_ids, self.after_ids]:
            validate_input_ids(values, self.expected, response)

    def diagnostic(self, error, response):
        return {"case_id": self.case_id, "error_type": type(error).__name__,
                "error": str(error), "expected_input_ids": self.expected,
                "record_input_ids": self.record_ids, "before_input_ids": self.before_ids,
                "after_input_ids": self.after_ids,
                "record_difference": token_difference(self.record_ids, self.expected),
                "before_difference": token_difference(self.before_ids, self.expected),
                "after_difference": token_difference(self.after_ids, self.expected),
                "logits": self.logits, "response": response}


def diagnostic_json(value):
    # Invalid numerical results must remain diagnosable without writing NaN or
    # Infinity as JSON numbers or masking the original validation exception.
    if isinstance(value, float) and not math.isfinite(value):
        return {"nonfinite": repr(value)}
    if isinstance(value, dict):
        return {key: diagnostic_json(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [diagnostic_json(item) for item in value]
    return value


def reference(args):
    # Set bounds before importing numerical runtimes.
    for name in ["OMP_NUM_THREADS", "OPENBLAS_NUM_THREADS", "VECLIB_MAXIMUM_THREADS", "MKL_NUM_THREADS"]:
        os.environ[name] = str(args.threads)
    import torch
    import transformers
    torch.set_num_threads(args.threads)
    torch.set_num_interop_threads(1)
    if args.device == "mps":
        if not math.isfinite(args.mps_memory_gib) or not 0 < args.mps_memory_gib <= 64:
            raise ValueError("MPS allocation limit must be positive and at most 64 GiB")
        if "gpu" not in os.environ.get("OPENAGENTS_LEASES", "").split(","):
            raise ValueError("MPS reference execution requires a GPU lease")
        limit = min(1.0, args.mps_memory_gib * 1024 ** 3 / torch.mps.recommended_max_memory())
        torch.mps.set_per_process_memory_fraction(limit)
    checkpoint = args.checkpoint.resolve()
    if not math.isfinite(args.max_model_gib) or args.max_model_gib <= 0 or args.max_input_tokens <= 0:
        raise ValueError("model and input bounds must be positive and finite")
    checkpoint_record = validate_checkpoint(
        checkpoint, args.max_model_gib / (2 if args.dtype == "float32" else 1)
    )
    spec = importlib.util.spec_from_file_location("clef_image_reference", checkpoint / "joint_schema_model.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    cases = load_suite(args)
    identities = {case["id"] for case, _, _ in cases}
    if args.case and not set(args.case) <= identities:
        raise ValueError("selected case does not exist")
    processor = transformers.AutoProcessor.from_pretrained(checkpoint, local_files_only=True)
    encoded_ids = {}
    for case, request, _ in cases:
        encoded = module.encode_record(processor.tokenizer, request, processor=processor)
        if len(encoded.input_ids) > args.max_input_tokens:
            raise ValueError(f"case {case['id']} exceeds the reference token bound")
        encoded_ids[case["id"]] = list(encoded.input_ids)
    dtype = {"bfloat16": torch.bfloat16, "float32": torch.float32}[args.dtype]
    identity = {
        "kind": "cloudflare_reference", "checkpoint": str(checkpoint),
        "reference_source_sha256": sha(checkpoint / "joint_schema_model.py"),
        "head_sha256": sha(checkpoint / "joint_head.safetensors"),
        "head_config_sha256": sha(checkpoint / "joint_head_config.json"),
        "config_sha256": sha(checkpoint / "config.json"),
        "tokenizer_sha256": sha(checkpoint / "tokenizer.json"),
        "tokenizer_config_sha256": sha(checkpoint / "tokenizer_config.json"),
        "processor_sha256": sha(checkpoint / "processor_config.json"),
        "cases_sha256": suite_sha(args), "device": args.device, "dtype": args.dtype,
        "suite": "text" if args.text_suite else "images", "model": args.model,
        "expected_case_ids": [case["id"] for case, _, _ in cases],
        "attention": "eager", "threads": args.threads,
        "mps_memory_gib": args.mps_memory_gib if args.device == "mps" else None,
        "torch": torch.__version__, "transformers": transformers.__version__,
        "accelerate": importlib.metadata.version("accelerate"),
        **checkpoint_record, "max_input_tokens": args.max_input_tokens,
    }
    if (checkpoint / "verified.json").is_file():
        identity["verified_manifest_sha256"] = sha(checkpoint / "verified.json")
    if (checkpoint / "expected.json").is_file():
        source = json.loads((checkpoint / "expected.json").read_text())
        identity["repository"] = source["repository"]
        identity["revision"] = source["revision"]
    if args.output.exists():
        if not args.resume:
            raise ValueError("output exists; use --resume to continue the same reference")
        record = json.loads(args.output.read_text())
        if any(record.get(key) != value for key, value in identity.items()):
            raise ValueError("resume reference has different weights, cases, or execution settings")
    else:
        record = {**identity, "load_seconds": [], "completed": False, "cases": []}
    previous = {row["id"]: row for row in record["cases"]}
    if len(previous) != len(record["cases"]) or not set(previous) <= identities:
        raise ValueError("resume reference has duplicate or unknown cases")
    for case, request, digests in cases:
        if case["id"] in previous:
            row = previous[case["id"]]
            validate_response(row["response"], request)
            if row["image_sha256"] != digests or row.get("source_request_sha256") != case.get("source_request_sha256"):
                raise ValueError("resume case contents changed")
            validate_logits(row["logits"], request)
            validate_input_ids(row["input_ids"], encoded_ids[case["id"]], row["response"])
            selected = {key: top_answer(answer) for key, answer in row["response"]["answers"].items()}
            if selected != row["top_answers"]:
                raise ValueError("resume answers are inconsistent")
            row["matches_expected"] = case["expected"] is None or selected == case["expected"]
    record["completed"] = len(record["cases"]) == len(cases)
    record["all_match_expected"] = all(row["matches_expected"] for row in record["cases"])
    pending = [(case, request, digests) for case, request, digests in cases
               if case["id"] not in previous and (not args.case or case["id"] in args.case)]
    if not pending:
        write(args.output, record)
        print("all selected reference cases are already complete", flush=True)
        return 0 if record["all_match_expected"] else 1
    write(args.output, record)
    started = time.monotonic()
    model, processor = module.load_release_model(
        checkpoint, device=args.device, dtype=dtype, local_files_only=True,
        attn_implementation="eager", low_cpu_mem_usage=True,
    )
    record["load_seconds"].append(time.monotonic() - started)
    captured = ReferenceCapture()
    pre_hook = model.register_forward_pre_hook(captured.before)
    hook = model.register_forward_hook(captured.after)
    for case, request, digests in pending:
        started = time.monotonic()
        captured.begin(case["id"], encoded_ids[case["id"]])
        response = None
        try:
            with torch.inference_mode():
                response = module.systemone(model, processor, request)
            validate_response(response, request)
            captured.validate(response)
            validate_logits(captured.logits, request)
        except Exception as error:
            failure = args.output.with_name(
                f"{args.output.stem}.failure-{case['id']}-{time.time_ns()}.json"
            )
            write(failure, diagnostic_json(captured.diagnostic(error, response)))
            print(f"reference case {case['id']} failed; diagnostic: {failure}", file=sys.stderr, flush=True)
            hook.remove()
            pre_hook.remove()
            raise
        selected = {key: top_answer(answer) for key, answer in response["answers"].items()}
        row = {"id": case["id"], "image_sha256": digests, "response": response,
               "top_answers": selected, "expected": case["expected"],
               "matches_expected": case["expected"] is None or selected == case["expected"],
               "source_request_sha256": case.get("source_request_sha256"),
               "input_ids": captured.after_ids, "logits": captured.logits,
               "seconds": time.monotonic() - started}
        record["cases"].append(row)
        write(args.output, record)
        print(json.dumps({"case": case["id"], "top_answers": selected,
                          "matches_expected": row["matches_expected"], "seconds": row["seconds"]}), flush=True)
    hook.remove()
    pre_hook.remove()
    record["completed"] = len(record["cases"]) == len(cases)
    record["all_match_expected"] = all(row["matches_expected"] for row in record["cases"])
    write(args.output, record)
    return 0 if record["all_match_expected"] else 1


def native(args):
    expected = json.loads(args.reference.read_text())
    if expected.get("kind") != "cloudflare_reference" or expected.get("dtype") != "bfloat16" or not expected.get("completed"):
        raise ValueError("native acceptance needs a completed official BF16 reference")
    if expected["cases_sha256"] != suite_sha(args):
        raise ValueError("reference uses a different case set")
    previous = {row["id"]: row for row in expected["cases"]}
    cases = load_suite(args)
    if len(previous) != len(expected["cases"]) or set(previous) != {case["id"] for case, _, _ in cases}:
        raise ValueError("reference is incomplete or contains different cases")
    record = {"kind": "native_image_parity", "reference_sha256": sha(args.reference),
              "cases_sha256": suite_sha(args), "endpoint": args.endpoint, "cases": [],
              "suite": "text" if args.text_suite else "images",
              "acceptance": ("all top answers, identical token counts, and maximum probability difference <= 0.02"
                             if args.text_suite else "all top answers, identical token counts, and HTTP refusals")}
    for case, request, digests in cases:
        reference_row = previous[case["id"]]
        if reference_row["image_sha256"] != digests:
            raise ValueError(f"images changed for {case['id']}")
        reference_response = reference_row["response"]
        validate_response(reference_response, request)
        reference_answers = {key: top_answer(answer) for key, answer in reference_response["answers"].items()}
        if reference_answers != reference_row["top_answers"] or (
            case["expected"] is not None and reference_answers != case["expected"]
        ):
            raise ValueError(f"reference does not match declared image contents for {case['id']}")
        started = time.monotonic()
        status, response = post(args.endpoint, request, args.timeout, case.get("request_body"))
        if status != 200:
            raise ValueError(f"native case {case['id']} failed with HTTP {status}: {response}")
        validate_response(response, request)
        selected = {key: top_answer(answer) for key, answer in response["answers"].items()}
        differences = []
        comparisons = {}
        for key, answer in response["answers"].items():
            other = reference_response["answers"][key]
            if answer["type"] == "noul":
                delta = abs(answer["noul"] - other["noul"])
            else:
                delta = max(abs(value - other["probabilities"][option])
                            for option, value in answer["probabilities"].items())
            differences.append(delta)
            comparisons[key] = {"type": answer["type"], "native_top_answer": top_answer(answer),
                                "reference_top_answer": top_answer(other),
                                "matches_reference": top_answer(answer) == top_answer(other),
                                "native_top_two_margin": top_two_margin(answer),
                                "reference_top_two_margin": top_two_margin(other),
                                "maximum_probability_difference": delta}
        row = {"id": case["id"], "response": response, "top_answers": selected,
               "matches_reference": selected == reference_row["top_answers"],
               "matches_expected": case["expected"] is None or selected == case["expected"],
               "tokens_match": response["usage"]["input_tokens"] == reference_response["usage"]["input_tokens"],
               "question_comparisons": comparisons,
               "maximum_probability_difference": max(differences), "seconds": time.monotonic() - started}
        record["cases"].append(row)
        write(args.output, record)
        print(json.dumps({key: value for key, value in row.items() if key != "response"}), flush=True)
    record["http_refusals"] = [] if args.text_suite else check_refusals(args, cases[0][1])
    summarize_native(record, bool(args.text_suite))
    write(args.output, record)
    return 0 if record["passed"] else 1


def summarize_native(record, text_suite):
    if not record["cases"]:
        raise ValueError("native acceptance has no cases")
    record["top_answers_and_tokens_match"] = all(
        row["matches_reference"] and row["matches_expected"] and row["tokens_match"] for row in record["cases"]
    )
    record["maximum_probability_difference"] = max(row["maximum_probability_difference"] for row in record["cases"])
    if text_suite:
        record["original_M1_numeric_gate"] = (
            all(row["matches_reference"] for row in record["cases"])
            and record["maximum_probability_difference"] <= 0.02
        )
    record["passed"] = (all(row["passed"] for row in record["http_refusals"])
                        and record["top_answers_and_tokens_match"]
                        and record.get("original_M1_numeric_gate", True))


def validate_logits(logits, request):
    counts = [2 if q["type"] == "noul" else len(q["criteria"]) for q in request["questions"].values()]
    if len(logits) != len(counts) or any(
        len(row) != count or any(not math.isfinite(value) for value in row)
        for row, count in zip(logits, counts)
    ):
        raise ValueError("reference logits have incomplete options or nonfinite values")


def validate_input_ids(actual, expected, response):
    if any(type(token) is not int or token < 0 for token in actual) or actual != expected:
        raise ValueError("reference token IDs disagree with fresh encoding")
    if len(actual) != response["usage"]["input_tokens"]:
        raise ValueError("reference token count is inconsistent")


def top_two_margin(answer):
    probabilities = ([answer["noul"], 1.0 - answer["noul"]] if answer["type"] == "noul"
                     else list(answer["probabilities"].values()))
    ordered = sorted(probabilities, reverse=True)
    return ordered[0] - ordered[1] if len(ordered) > 1 else None


def capture_self_test():
    from types import SimpleNamespace

    def rejects(operation):
        try:
            operation()
        except ValueError:
            return
        raise AssertionError("invalid reference capture passed acceptance")

    expected = [11, 22, 33]
    response = {"usage": {"input_tokens": 3}}
    capture = ReferenceCapture(ids=lambda batch: batch["input_ids"], logits=lambda output: output)

    def fresh():
        capture.begin("text-034", expected)
        return {"input_ids": expected.copy(),
                "records": [SimpleNamespace(input_ids=expected.copy())]}

    batch = fresh()
    capture.before(None, (batch,))
    rejects(lambda: capture.before(None, (batch,)))
    capture.after(None, (batch,), [[0.1, 0.2]])
    capture.validate(response)
    rejects(lambda: capture.after(None, (batch,), [[0.1, 0.2]]))
    batch["input_ids"][0] = 99
    batch["records"][0].input_ids[0] = 88
    assert capture.before_ids == capture.after_ids == capture.record_ids == expected
    assert token_difference(capture.after_ids, expected) is None

    batch = fresh()
    assert capture.before_ids is None and capture.after_ids is None and capture.logits is None
    rejects(lambda: capture.validate(response))
    rejects(lambda: capture.after(None, (batch,), [[0.1, 0.2]]))
    for changed in ([11, 99, 33], [11, 22], [11, 22, 33, 44], [True, 22, 33]):
        batch = fresh()
        batch["input_ids"] = list(changed)
        rejects(lambda: capture.before(None, (batch,)))
        batch = fresh()
        batch["records"][0].input_ids = list(changed)
        rejects(lambda: capture.before(None, (batch,)))
        batch = fresh()
        capture.before(None, (batch,))
        batch["input_ids"][:] = changed
        capture.after(None, (batch,), [[0.1, 0.2]])
        rejects(lambda: capture.validate(response))
        diagnostic = capture.diagnostic(ValueError("token mismatch"), response)
        assert diagnostic["case_id"] == "text-034"
        assert diagnostic["before_input_ids"] == expected
        assert diagnostic["after_input_ids"] == changed
        assert diagnostic["before_difference"] is None
        assert diagnostic["after_difference"] is not None
    batch = fresh()
    batch["records"] = []
    rejects(lambda: capture.before(None, (batch,)))
    assert token_difference(None, expected) == {"captured": False, "expected_length": 3}
    assert token_difference([11, 99, 33], expected)["index"] == 1
    assert token_difference([11, 22], expected)["index"] == 2
    encoded = json.dumps(diagnostic_json({"logits": [float("nan"), float("inf"), -float("inf")]}),
                         allow_nan=False)
    assert json.loads(encoded)["logits"] == [
        {"nonfinite": "nan"}, {"nonfinite": "inf"}, {"nonfinite": "-inf"}]


def self_test():
    capture_self_test()
    request = {"model": "clef", "questions": {
        "color": {"type": "choice", "criteria": {"red": "Red", "blue": "Blue"}},
        "red": {"type": "noul"},
        "amount": {"type": "score", "criteria": ["None", "All"]},
    }}
    valid = {"model": "clef", "usage": {"input_tokens": 123, "output_tokens": 0}, "answers": {
        "color": {"type": "choice", "choice": "red", "confidence": 0.9,
                  "probabilities": {"red": 0.9, "blue": 0.1}},
        "red": {"type": "noul", "noul": 0.9},
        "amount": {"type": "score", "confidence": 0.9, "score": 0.9,
                   "probabilities": {"0": 0.1, "1": 0.9}},
    }}
    validate_response(valid, request)
    changes = [
        lambda r: r["answers"].pop("red"),
        lambda r: r["answers"]["red"].update(type="score"),
        lambda r: r["answers"]["red"].update(noul=float("nan")),
        lambda r: r["answers"]["amount"].update(score=float("inf")),
        lambda r: r["answers"]["color"]["probabilities"].pop("blue"),
        lambda r: r["answers"]["color"]["probabilities"].update(red=float("nan")),
        lambda r: r["answers"]["color"].update(choice="green"),
        lambda r: r["answers"]["color"].update(choice="blue"),
        lambda r: r["answers"]["color"].update(confidence=0.1),
        lambda r: r["answers"]["amount"].update(score=0.1),
        lambda r: r["answers"]["amount"].update(confidence=0.1),
        lambda r: r["usage"].update(input_tokens=0),
        lambda r: r["usage"].update(output_tokens=False),
        lambda r: r["usage"].update(cached_input_tokens=1),
        lambda r: r.update(cached=True),
        lambda r: r.update(model="other"),
    ]
    for change in changes:
        invalid = json.loads(json.dumps(valid))
        change(invalid)
        try:
            validate_response(invalid, request)
        except (ValueError, KeyError, TypeError):
            continue
        raise AssertionError("invalid response passed acceptance")
    ids = list(range(123))
    validate_input_ids(ids, ids, valid)
    for actual, expected in [(ids[::-1], ids), (ids[:-1], ids[:-1]), ([True] * 123, [True] * 123)]:
        try:
            validate_input_ids(actual, expected, valid)
        except ValueError:
            continue
        raise AssertionError("changed or incomplete token IDs passed acceptance")
    for text_suite, delta, top_match, tokens_match, passes in [
        (True, 0.02, True, True, True),
        (True, 0.0201, True, True, False),
        (True, 0.0, False, True, False),
        (True, 0.0, True, False, False),
        (False, 0.2, True, True, True),
    ]:
        record = {"http_refusals": [], "cases": [{"matches_reference": top_match,
                  "matches_expected": True, "tokens_match": tokens_match,
                  "maximum_probability_difference": delta}]}
        summarize_native(record, text_suite)
        assert record["passed"] is passes
        assert ("original_M1_numeric_gate" in record) is text_suite
    print(f"response validation passed: valid response and {len(changes)} rejection cases")
    print("token identity and separate text/image acceptance gates passed")
    print("per-case capture ownership, strict before/after checks, and failure diagnostics passed")
    return 0


def device_check(args):
    if args.device == "mps" and "gpu" not in os.environ.get("OPENAGENTS_LEASES", "").split(","):
        raise ValueError("the MPS capability check requires a GPU lease")
    for name in ["OMP_NUM_THREADS", "OPENBLAS_NUM_THREADS", "VECLIB_MAXIMUM_THREADS", "MKL_NUM_THREADS"]:
        os.environ[name] = "2"
    os.environ["PYTORCH_ENABLE_MPS_FALLBACK"] = "0"
    import torch
    from transformers.models.qwen3_5.modeling_qwen3_5 import torch_chunk_gated_delta_rule
    torch.set_num_threads(2)
    torch.set_num_interop_threads(1)
    torch.manual_seed(947)
    if args.device == "mps":
        torch.mps.set_per_process_memory_fraction(0.02)
    inputs = [torch.randn(1, 65, 2, 128, dtype=torch.bfloat16) * 0.1 for _ in range(3)]
    inputs += [-torch.rand(1, 65, 2, dtype=torch.bfloat16),
               torch.rand(1, 65, 2, dtype=torch.bfloat16)]
    with torch.inference_mode():
        expected, state = torch_chunk_gated_delta_rule(
            *inputs, output_final_state=True, use_qk_l2norm_in_kernel=True
        )
        actual, actual_state = torch_chunk_gated_delta_rule(
            *(value.to(args.device) for value in inputs),
            output_final_state=True, use_qk_l2norm_in_kernel=True
        )
        kernel = torch.nn.Conv3d(3, 4, (2, 16, 16), stride=(2, 16, 16), dtype=torch.bfloat16)
        pixels = torch.randn(1, 3, 2, 32, 32, dtype=torch.bfloat16)
        convolution = kernel(pixels)
        actual_convolution = kernel.to(args.device)(pixels.to(args.device)).cpu()
    deltas = {
        "delta_output": (actual.cpu().float() - expected.float()).abs().max().item(),
        "delta_state": (actual_state.cpu().float() - state.float()).abs().max().item(),
        "vision_conv3d": (actual_convolution.float() - convolution.float()).abs().max().item(),
    }
    passed = all(math.isfinite(value) and value <= 0.02 for value in deltas.values())
    record = {"kind": "reference_device_capability", "device": args.device,
              "torch": torch.__version__, "maximum_absolute_differences": deltas,
              "passed": passed, "full_model_validation": False}
    if args.device == "mps":
        record["driver_allocated_bytes"] = torch.mps.driver_allocated_memory()
    write(args.output, record)
    print(json.dumps(record), flush=True)
    return 0 if passed else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subcommands = parser.add_subparsers(dest="mode", required=True)
    subcommands.add_parser("self-test", help="check result validation without models or networking")
    check = subcommands.add_parser("device-check", help="bounded DeltaNet and vision Conv3d capability check")
    check.add_argument("--device", choices=["cpu", "mps"], default="mps")
    check.add_argument("--output", type=Path, required=True)
    for mode in ["reference", "native"]:
        command = subcommands.add_parser(mode)
        command.add_argument("--cases", type=Path, default=DEFAULT_CASES)
        command.add_argument("--text-suite", type=Path, help="JSONL requests, preserving source number spellings")
        command.add_argument("--model", default="clef")
        command.add_argument("--output", type=Path, required=True)
        if mode == "reference":
            command.add_argument("--checkpoint", type=Path, required=True)
            command.add_argument("--device", choices=["cpu", "mps", "cuda"], default="cpu")
            command.add_argument("--dtype", choices=["bfloat16", "float32"], default="bfloat16")
            command.add_argument("--threads", type=int, choices=[1, 2], default=2)
            command.add_argument("--max-input-tokens", type=int, default=2048)
            command.add_argument("--max-model-gib", type=float, default=64)
            command.add_argument("--mps-memory-gib", type=float, default=60)
            command.add_argument("--case", action="append", help="run only this case ID (repeatable)")
            command.add_argument("--resume", action="store_true")
        else:
            command.add_argument("--reference", type=Path, required=True)
            command.add_argument("--endpoint", required=True)
            command.add_argument("--timeout", type=float, default=600)
    args = parser.parse_args()
    if args.mode == "self-test":
        return self_test()
    if args.mode == "device-check":
        return device_check(args)
    return reference(args) if args.mode == "reference" else native(args)


if __name__ == "__main__":
    raise SystemExit(main())
