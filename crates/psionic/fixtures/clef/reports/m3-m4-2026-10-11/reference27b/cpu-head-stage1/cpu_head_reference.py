#!/usr/bin/env python3
"""Diagnostic reference: official BF16 MPS trunk with official CPU f32 head.

This is a separate execution configuration, not a replacement acceptance gate.
All 40 text cases save immutable head inputs for later CPU-only replay.
"""
import argparse
from dataclasses import asdict
import hashlib
import importlib.metadata
import importlib.util
import json
import os
from pathlib import Path
import shutil
import sys
import time

ROOT = Path("repo")
BASE = Path("task-scratch/clef-m3-m4")
TOOLS = ROOT / "crates/psionic/fixtures/clef/tools"
sys.path.insert(0, str(TOOLS))
import image_parity as parity

VERIFIED_SHA = "4cc3f8a38f851984baa481857de737facf17f71c63b632ceda7d510e5ee751eb"
SUITE_SHA = "3958f5f3b9d09d7fac61a39e40c69e812ef09c39f06baae2ab0286e171ab19bc"
PRIOR_SHA = "5fededd9e1df8433b0ec4d4ac0b9d3f1f40c686e7700ee9af8a87ee562771867"
REVISION = "0b331204bb13fbd2ca93a64df1956f5b55478ce5"
GIB = 1024**3


class DiagnosticMismatch(ValueError):
    def __init__(self, message, details):
        super().__init__(message)
        self.details = details


def require(condition, message, details=None):
    if not condition:
        raise DiagnosticMismatch(message, details)


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024**2), b""):
            digest.update(block)
    return digest.hexdigest()


def signature(path):
    value = path.stat()
    return {"bytes": value.st_size, "inode": value.st_ino, "mtime_ns": value.st_mtime_ns, "ctime_ns": value.st_ctime_ns}


def frozen(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def tensor_bytes(torch, tensor):
    require(tensor.device.type == "cpu", "byte snapshot requires a CPU tensor")
    return tensor.detach().contiguous().view(torch.uint8).numpy().tobytes()


def tensor_identity(torch, tensor):
    value = tensor_bytes(torch, tensor)
    return {"dtype": str(tensor.dtype), "shape": list(tensor.shape), "bytes": len(value),
            "sha256": hashlib.sha256(value).hexdigest()}


def mps_copy(torch, tensor):
    """Detach device storage through immutable bytes, then make fresh CPU storage."""
    require(tensor.device.type == "mps", "trunk tensor is not on MPS")
    torch.mps.synchronize()
    transfer = tensor.detach().to(device="cpu", copy=True).contiguous()
    torch.mps.synchronize()
    immutable = tensor_bytes(torch, transfer)
    cpu = torch.frombuffer(bytearray(immutable), dtype=tensor.dtype).reshape(tuple(tensor.shape))
    require(tensor_bytes(torch, cpu) == immutable, "CPU transfer bytes changed")
    return cpu, immutable


def check_device_inputs(torch, batch, expected_ids, records_bytes):
    ids, ids_bytes = mps_copy(torch, batch["input_ids"])
    mask, mask_bytes = mps_copy(torch, batch["attention_mask"])
    require(ids.dtype == torch.int64 and ids.tolist() == [expected_ids], "MPS input IDs changed",
            {"expected_ids": expected_ids, "actual_ids": ids.tolist(), "dtype": str(ids.dtype)})
    require(mask.dtype == torch.int64 and list(mask.shape) == [1, len(expected_ids)]
            and mask.tolist() == [[1]*len(expected_ids)], "MPS attention mask changed",
            {"expected_mask": [[1]*len(expected_ids)], "actual_mask": mask.tolist(), "dtype": str(mask.dtype)})
    require(frozen([asdict(record) for record in batch["records"]]) == records_bytes, "encoded records changed")
    return {"input_ids_sha256": hashlib.sha256(ids_bytes).hexdigest(),
            "attention_mask_sha256": hashlib.sha256(mask_bytes).hexdigest(),
            "records_sha256": hashlib.sha256(records_bytes).hexdigest(), "exact": True}


class LexicalRows:
    """Read selected untied BF16 LM-head rows without an MPS gather kernel."""
    def __init__(self, checkpoint, index):
        self.path = checkpoint / index["weight_map"]["lm_head.weight"]
        with self.path.open("rb") as source:
            length = int.from_bytes(source.read(8), "little")
            require(0 < length <= 16*1024**2, "invalid lexical shard header size")
            header = json.loads(source.read(length))
        value = header["lm_head.weight"]
        require(value["dtype"] == "BF16" and value["shape"] == [248320, 5120], "unexpected output embedding tensor")
        start, end = value["data_offsets"]
        require(end-start == 248320*5120*2 and end+length+8 <= self.path.stat().st_size, "invalid lexical tensor range")
        self.start = length+8+start
        self.row_bytes = 5120*2
        self.stat = signature(self.path)

    def read(self, torch, tokens):
        require(tokens == sorted(set(tokens)) and 0 < len(tokens) <= 2048, "invalid lexical token set")
        require(all(type(t) is int and 0 <= t < 248320 for t in tokens), "lexical token outside vocabulary")
        require(signature(self.path) == self.stat, "lexical checkpoint changed")
        with self.path.open("rb") as source:
            rows = []
            for token in tokens:
                source.seek(self.start+token*self.row_bytes)
                row = source.read(self.row_bytes)
                require(len(row) == self.row_bytes, "short lexical row read")
                rows.append(row)
        immutable = b"".join(rows)
        result = torch.frombuffer(bytearray(immutable), dtype=torch.bfloat16).reshape(len(tokens), 5120)
        require(tensor_bytes(torch, result) == immutable, "lexical row bytes changed")
        return result, immutable


def validate_artifacts(checkpoint, suite, prior):
    require(sha(checkpoint / "verified.json") == VERIFIED_SHA, "verified artifact manifest changed")
    require(sha(suite) == SUITE_SHA, "text suite changed")
    require(sha(prior) == PRIOR_SHA, "old reference changed")
    manifest = json.loads((checkpoint / "verified.json").read_text())
    require(manifest["complete"] and manifest["repository"] == "Cloudflare/clef" and manifest["revision"] == REVISION,
            "artifact identity differs")
    observed = {}
    for entry in manifest["files"]:
        name = entry["filename"]
        require(Path(name).name == name, "artifact is not a direct child")
        path = checkpoint / name
        require(path.stat().st_size == entry["bytes"], "artifact size changed: "+name)
        # Large shards already have complete SHA-256 verification in verified.json.
        # Record exact file state now and check it again after every request.
        checked = not name.startswith("model-")
        if checked:
            require(sha(path) == entry["sha256"], "artifact hash changed: "+name)
        observed[name] = {"sha256": entry["sha256"], "hash_rechecked_this_run": checked, **signature(path)}
    config = json.loads((checkpoint / "config.json").read_text())
    require(config.get("tie_word_embeddings") is False and config["text_config"].get("tie_word_embeddings") is False,
            "direct lexical rows require the released untied output embedding")
    return observed, parity.validate_checkpoint(checkpoint, 60)


def assert_file_state(checkpoint, observed):
    for name, entry in observed.items():
        require(signature(checkpoint / name) == {key: entry[key] for key in ("bytes", "inode", "mtime_ns", "ctime_ns")},
                "checkpoint changed during reference run: "+name)


def case_forward(torch, official, text_model, head, processor, lexical, encoded, case, request, output):
    expected_ids = list(encoded.input_ids)
    require(0 < len(expected_ids) <= 2048 and encoded.media is None, "case is outside text-only input bounds")
    records_bytes = frozen([asdict(encoded)])
    batch = official.collate_records([encoded], processor.tokenizer.pad_token_id, torch.device("mps"))
    require(not batch.get("media"), "unexpected media tensors")
    before = check_device_inputs(torch, batch, expected_ids, records_bytes)
    started = time.monotonic()
    with torch.inference_mode():
        # This is the exact text-only branch of official ClefModel.forward.
        result = text_model(input_ids=batch["input_ids"], attention_mask=batch["attention_mask"],
                            use_cache=False, return_dict=True)
        torch.mps.synchronize()
        trunk_seconds = time.monotonic()-started
        after_trunk = check_device_inputs(torch, batch, expected_ids, records_bytes)
        hidden_bf16, hidden_bytes = mps_copy(torch, result.last_hidden_state)
        require(hidden_bf16.dtype == torch.bfloat16 and list(hidden_bf16.shape) == [1, len(expected_ids), 5120]
                and bool(torch.isfinite(hidden_bf16).all()), "invalid trunk hidden rows")
        tokens = sorted({token for q in encoded.questions for first, end in q.option_spans for token in expected_ids[first:end]})
        lexical_bf16, lexical_bytes = lexical.read(torch, tokens)
        require(bool(torch.isfinite(lexical_bf16).all()), "nonfinite lexical rows")
        remap = {token: index for index, token in enumerate(tokens)}
        compact_ids = [remap.get(token, 0) for token in expected_ids]
        head_record = official.EncodedRecord(input_ids=tuple(compact_ids), questions=encoded.questions, record_id=encoded.record_id)
        head_records_bytes = frozen([asdict(head_record)])
        inputs = {"hidden": hidden_bf16.float(), "lexical": lexical_bf16.float(),
                  "input_ids": torch.tensor([compact_ids], dtype=torch.long, device="cpu"),
                  "attention_mask": torch.ones((1, len(expected_ids)), dtype=torch.long, device="cpu")}
        identities = {name: tensor_identity(torch, value) for name, value in inputs.items()}
        from safetensors.torch import save_file
        saved = output / (case["id"]+"-head-inputs.safetensors")
        require(not saved.exists(), "case input evidence already exists")
        save_file({"hidden": hidden_bf16, "lexical": lexical_bf16,
                   "input_ids": torch.tensor([expected_ids], dtype=torch.long),
                   "lexical_token_ids": torch.tensor(tokens, dtype=torch.long),
                   "remapped_input_ids": inputs["input_ids"], "attention_mask": inputs["attention_mask"]}, str(saved))
        head_started = time.monotonic()
        raw = head(inputs["hidden"], inputs["input_ids"], inputs["attention_mask"], [head_record], inputs["lexical"])[0]
        logits = [row.detach().float().tolist() for row in raw]
        head_seconds = time.monotonic()-head_started
        after_head = {name: tensor_identity(torch, value) for name, value in inputs.items()}
        require(after_head == identities, "CPU head mutated its tensor inputs", {"before": identities, "after": after_head})
        require(frozen([asdict(head_record)]) == head_records_bytes and frozen([asdict(encoded)]) == records_bytes,
                "CPU head mutated records")
        require(tensor_bytes(torch, hidden_bf16) == hidden_bytes and tensor_bytes(torch, lexical_bf16) == lexical_bytes,
                "CPU head changed original BF16 input storage")
        # A second device read also checks that host-side head work did not alter
        # unified-memory trunk inputs or outputs through an unexpected alias.
        after = check_device_inputs(torch, batch, expected_ids, records_bytes)
        _, hidden_after_bytes = mps_copy(torch, result.last_hidden_state)
        require(hidden_after_bytes == hidden_bytes, "MPS hidden rows changed during CPU head execution",
                {"before_sha256": hashlib.sha256(hidden_bytes).hexdigest(),
                 "after_sha256": hashlib.sha256(hidden_after_bytes).hexdigest()})
        _, lexical_after_bytes = lexical.read(torch, tokens)
        require(lexical_after_bytes == lexical_bytes, "checkpoint lexical rows changed")
        require(before == after_trunk == after, "MPS input snapshots changed")
        require(len(logits) == len(encoded.questions), "head output question count differs")
        answers = {}
        for question, values in zip(encoded.questions, logits, strict=True):
            require(len(values) == len(question.option_ids), "head output option count differs")
            probs = torch.tensor(values, dtype=torch.float32).softmax(-1).tolist()
            answers[question.question_id] = official.systemone_answer(
                request["questions"][question.question_id], dict(zip(question.option_ids, probs, strict=True)))
    response = {"model": request["model"], "answers": answers,
                "usage": {"input_tokens": len(expected_ids), "output_tokens": 0}}
    parity.validate_response(response, request)
    parity.validate_logits(logits, request)
    parity.validate_input_ids(expected_ids, list(encoded.input_ids), response)
    return {"id": case["id"], "image_sha256": [], "source_request_sha256": case["source_request_sha256"],
            "request_body": case["request_body"], "input_ids": expected_ids, "records": [asdict(encoded)],
            "head_records": [asdict(head_record)], "response": response, "logits": logits,
            "top_answers": {key: parity.top_answer(value) for key, value in answers.items()},
            "expected": None, "matches_expected": True, "seconds": time.monotonic()-started,
            "trunk_seconds": trunk_seconds, "cpu_head_seconds": head_seconds,
            "head_inputs": {"file": saved.name, "bytes": saved.stat().st_size, "sha256": sha(saved)},
            "immutable_checks": {"before_trunk": before, "after_trunk": after_trunk, "after_head": after,
                "cpu_head_before": identities, "cpu_head_after": after_head, "all_exact": True,
                "original_bf16_hidden_sha256": hashlib.sha256(hidden_bytes).hexdigest(),
                "original_bf16_lexical_sha256": hashlib.sha256(lexical_bytes).hexdigest(),
                "mps_hidden_unchanged_after_head": True, "lexical_reread_exact": True}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    leases = set(os.environ.get("OPENAGENTS_LEASES", "").split(","))
    require({"memory", "gpu"} <= leases and "quiet" not in leases, "requires GPU and memory leases; quiet is forbidden")
    require(os.environ.get("CLEF_CPU_HEAD_REFERENCE_GUARDED") == "1", "run through the task-owned guard")
    require(not args.output_dir.exists(), "output exists; preserve the previous reference")
    require(shutil.disk_usage(args.output_dir.parent).free >= 3*GIB, "less than 3 GiB available for bounded input evidence")
    args.output_dir.mkdir(parents=True)
    checkpoint = BASE / "artifacts/clef-bf16"
    suite = ROOT / "crates/psionic/fixtures/clef/e2e/requests.jsonl"
    prior_path = BASE / "reference27b/text-mps-bf16.json"
    destination = args.output_dir / "reference.json"
    report = {"kind": "diagnostic_bf16_mps_trunk_cpu_f32_head", "status": "validating", "completed": False,
              "diagnostic_only": True, "cases": [], "suite": "text", "model": "clef", "device": "mps+cpu",
              "dtype": "trunk:bfloat16,head:float32", "trunk_device": "mps", "trunk_dtype": "bfloat16",
              "head_device": "cpu", "head_dtype": "float32", "threads": 2, "attention": "eager",
              "mps_fallback": False, "mps_memory_gib": 60, "max_input_tokens": 2048,
              "repository": "Cloudflare/clef", "revision": REVISION,
              "script_sha256": sha(Path(__file__)), "parity_helper_sha256": sha(TOOLS / "image_parity.py"),
              "old_reference_sha256": PRIOR_SHA, "cases_sha256": SUITE_SHA, "verified_manifest_sha256": VERIFIED_SHA,
              "conditions": "Shared machine. Separate diagnostic reference; preserves old goldens and acceptance tolerances.",
              "lexical_source": "Untied BF16 lm_head.weight rows read directly from the pinned checkpoint; no MPS gather.",
              "transfer_semantics": "MPS synchronize, explicit CPU copy, immutable byte snapshot, fresh bytearray-backed CPU tensor."}
    parity.write(destination, report)
    active_case = None
    try:
        observed, checkpoint_record = validate_artifacts(checkpoint, suite, prior_path)
        report.update(checkpoint_record, artifacts=observed)
        for name, key in (("joint_schema_model.py", "reference_source_sha256"), ("joint_head.safetensors", "head_sha256"),
                          ("joint_head_config.json", "head_config_sha256"), ("config.json", "config_sha256"),
                          ("tokenizer.json", "tokenizer_sha256"), ("tokenizer_config.json", "tokenizer_config_sha256"),
                          ("processor_config.json", "processor_sha256")):
            report[key] = observed[name]["sha256"]
        for name in ("OMP_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "VECLIB_MAXIMUM_THREADS", "RAYON_NUM_THREADS"):
            require(os.environ.get(name) == "2", "CPU thread bounds missing")
        import torch
        import transformers
        from safetensors.torch import load_file
        torch.set_num_threads(2)
        torch.set_num_interop_threads(1)
        torch.set_default_device("cpu")
        require(str(torch.__version__) == "2.9.1" and transformers.__version__ == "5.10.2", "reference runtime version changed")
        require(torch.backends.mps.is_available(), "MPS is unavailable")
        torch.mps.set_per_process_memory_fraction(min(1.0, 60*GIB/torch.mps.recommended_max_memory()))
        report.update(torch=str(torch.__version__), transformers=transformers.__version__,
                      accelerate=importlib.metadata.version("accelerate"))
        spec = importlib.util.spec_from_file_location("official_cpu_head_reference", checkpoint / "joint_schema_model.py")
        official = importlib.util.module_from_spec(spec)
        sys.modules[spec.name] = official
        spec.loader.exec_module(official)
        processor = transformers.AutoProcessor.from_pretrained(checkpoint, local_files_only=True)
        cases = parity.load_suite(argparse.Namespace(text_suite=suite, model="clef"))
        require([row[0]["id"] for row in cases] == [f"text-{i:03d}" for i in range(40)], "expected exactly 40 ordered text cases")
        report["expected_case_ids"] = [row[0]["id"] for row in cases]
        previous = {row["id"]: row for row in json.loads(prior_path.read_text())["cases"]}
        encoded = {}
        for case, request, _ in cases:
            value = official.encode_record(processor.tokenizer, request, processor=processor)
            require(len(value.input_ids) <= 2048 and value.media is None, "input exceeds text-only bound")
            require(list(value.input_ids) == previous[case["id"]]["input_ids"], "token IDs differ from old reference")
            encoded[case["id"]] = value
        head_config = json.loads((checkpoint / "joint_head_config.json").read_text())
        require(head_config["hidden_size"] == 5120, "head width differs")
        head = official.JointSchemaHead(**head_config)
        state = load_file(str(checkpoint / "joint_head.safetensors"), device="cpu")
        head.load_state_dict(state, strict=True)
        del state
        head = head.to(device="cpu", dtype=torch.float32).eval()
        index = json.loads((checkpoint / "model.safetensors.index.json").read_text())
        lexical = LexicalRows(checkpoint, index)
        report["status"] = "loading"
        parity.write(destination, report)
        began = time.monotonic()
        # Same loader and options as load_release_model; keep the official head
        # on CPU instead of first placing it on MPS.
        backbone = transformers.Qwen3_5ForConditionalGeneration.from_pretrained(
            checkpoint, dtype=torch.bfloat16, device_map={"": "mps"}, local_files_only=True,
            attn_implementation="eager", low_cpu_mem_usage=True)
        backbone.config.use_cache = False
        backbone.eval()
        base_model = backbone.get_base_model() if hasattr(backbone, "get_base_model") else backbone
        text_model = base_model.model
        if hasattr(text_model, "language_model"):
            text_model = text_model.language_model
        require(list(base_model.get_output_embeddings().weight.shape) == [248320, 5120], "loaded output embedding differs")
        require(all(p.device.type == "cpu" and p.dtype == torch.float32 for p in head.parameters()), "head placement differs")
        report.update(status="running", load_seconds=time.monotonic()-began)
        parity.write(destination, report)
        for case, request, _ in cases:
            active_case = case["id"]
            assert_file_state(checkpoint, observed)
            row = case_forward(torch, official, text_model, head, processor, lexical, encoded[active_case], case, request, args.output_dir)
            assert_file_state(checkpoint, observed)
            report["cases"].append(row)
            parity.write(args.output_dir / (active_case+".json"), row)
            parity.write(destination, report)
            print(json.dumps({"case": active_case, "status": "complete", "tokens": len(row["input_ids"]),
                              "immutable_checks_passed": True, "seconds": row["seconds"]}), flush=True)
        require(len(report["cases"]) == 40 and all(row["immutable_checks"]["all_exact"] for row in report["cases"]), "incomplete reference")
        report.update(status="complete", completed=True, all_immutable_checks_passed=True, all_match_expected=True)
    except BaseException as error:
        report.update(status="failed", failed_case=active_case, error_type=type(error).__name__, error=str(error))
        if getattr(error, "details", None) is not None:
            report["failure_details"] = error.details
        raise
    finally:
        parity.write(destination, report)


if __name__ == "__main__":
    main()
