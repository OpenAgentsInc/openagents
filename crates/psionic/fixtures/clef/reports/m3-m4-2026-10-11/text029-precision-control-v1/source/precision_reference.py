#!/usr/bin/env python3
"""Two fixed CPU precision controls against the saved text-029 f32 reference."""
import argparse
from dataclasses import asdict
import gc
import importlib.util
import json
import os
from pathlib import Path
import sys
import time
import types

HERE = Path(__file__).resolve().parent
RUN = HERE.parent
sys.path.insert(0, str(RUN))
import mps_reference as ref
from layer35_math import online_attention, fixtures as attention_fixtures
from layer0_math import delta_fixtures
from precision_hash import parameter_hashes, self_test as hash_self_test

MODES = ("B_projection_half", "C_projection_and_attention_half")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    leases = set(os.environ.get("OPENAGENTS_LEASES", "").split(","))
    ref.require({"memory", "disk"} <= leases and not {"gpu", "quiet", "build"} & leases
                and os.environ.get("CLEF_PRECISION_GUARDED") == "1", "CPU-only guarded control required")
    ref.require(not args.output_dir.exists(), "preserve previous output")
    args.output_dir.mkdir(parents=True)
    result = {
        "schema": "clef.text029-full-precision-control.v1", "status": "running", "completed": False,
        "gpu_executed": False, "artifact_weight_files_modified": False, "case": "text-029", "layers": [],
        "scope": "One fixed diagnostic case. Original 40/104 all-top and 0.02 probability acceptance remains unchanged and unmet.",
        "controls": {MODES[0]: "All nn.Linear weights and inputs rounded through f16; unchanged official f32 operators otherwise.",
                     MODES[1]: "Same projection rounding plus reviewed scaled-Q/K/V half conversion and 32-key half-exponential online attention."},
        "limitations": "These are precision-boundary controls, not bitwise Metal emulation. GEMM/norm reductions, transcendental functions, FMA and residual-in-GEMM accumulation differ. Delta remains official chunk64 in both controls.",
        "reference_A": "Previously saved unchanged independent CPU-f32 full run; no new A trunk execution.",
    }
    receipt = args.output_dir / "receipt.json"
    ref.write(receipt, result)
    reader = None
    original_eager = None
    try:
        import numpy as np
        import torch
        import transformers
        from safetensors.torch import load_file, save_file
        from transformers.models.qwen3_5 import modeling_qwen3_5 as modeling

        torch.set_num_threads(2)
        torch.set_num_interop_threads(1)
        torch.set_default_device("cpu")
        result["parameter_hash_self_test"] = hash_self_test(torch)
        ref.require(str(torch.__version__) == "2.9.1" and transformers.__version__ == "5.10.2"
                    and ref.sha(Path(modeling.__file__)) == ref.MODELING_SHA, "official runtime changed")
        ref.require(modeling.FusedRMSNormGated is None and modeling.causal_conv1d_fn is None
                    and modeling.chunk_gated_delta_rule is None and modeling.fused_recurrent_gated_delta_rule is None,
                    "unexpected optional kernels")
        for name, digest in ref.PINS.items():
            ref.require(ref.sha(ref.CPU / name) == digest, "independent importer changed")
        ref.require(ref.sha(ref.HF / "joint_schema_model.py") == ref.SOURCE_SHA
                    and ref.sha(ref.HF / "joint_head.safetensors") == ref.HEAD_SHA, "official head changed")
        config = modeling.Qwen3_5TextConfig.from_dict(json.loads((ref.HF / "config.json").read_text())["text_config"])
        config._attn_implementation = "eager"
        config.use_cache = False
        config.dtype = torch.float32
        ref.require(config.num_hidden_layers == 64 and config.hidden_size == 5120
                    and config.intermediate_size == 17408 and config.attention_dropout == 0
                    and config.layer_types == ["full_attention" if i % 4 == 3 else "linear_attention" for i in range(64)],
                    "architecture differs")
        cpu = json.loads((HERE / "cpu-1/results/receipt.json").read_text())
        native = json.loads((HERE / "native-1/results/receipt.json").read_text())
        observed = json.loads((HERE / "comparison-1/results/receipt.json").read_text())
        factorial = json.loads((HERE / "layer0-1/results/receipt.json").read_text())
        ref.require(cpu["completed"] and len(cpu["layers"]) == 64 and native["completed"]
                    and len(native["files"]) == 65 and observed["observer_consistency_screen"]["passed"]
                    and factorial["completed"] and factorial["variants"][0]["baseline_exact"], "prior proof incomplete")
        native_files = {v["name"]: v for v in native["files"]}

        def shape(value):
            ref.require(value.dtype == torch.float32 and value.device.type == "cpu"
                        and tuple(value.shape) == (1, 359, 5120) and bool(torch.isfinite(value).all()),
                        "hidden rows have invalid shape, dtype or values")

        def native_rows(name):
            path = HERE / "native-1/results/rows" / name
            ref.require(ref.sha(path) == native_files[name]["sha256"], "native rows changed")
            value = torch.from_numpy(np.fromfile(path, dtype="<f4").copy().reshape(1, 359, 5120))
            shape(value)
            return value

        def cpu_layer(index):
            row = cpu["layers"][index]
            path = HERE / "cpu-1/results" / row["file"]
            ref.require(row["layer"] == index and ref.sha(path) == row["file_sha256"], "CPU layer file changed")
            value = load_file(str(path), device="cpu")["hidden"]
            shape(value)
            ref.require(ref.digest(ref.cpu_bytes(torch, value)) == row["hidden_sha256"], "CPU hidden changed")
            return value

        spec = importlib.util.spec_from_file_location("precision_official_head", ref.HF / "joint_schema_model.py")
        official = importlib.util.module_from_spec(spec)
        sys.modules[spec.name] = official
        spec.loader.exec_module(official)
        processor = transformers.AutoProcessor.from_pretrained(ref.HF, local_files_only=True)
        request = (ref.NATIVE / "text-029.request.json").read_bytes()
        record = official.encode_record(processor.tokenizer, json.loads(request), processor=processor)
        native_encoded = json.loads((RUN / "encoder-source-run-1/native-encoded.json").read_text())["cases"][29]
        ref.require(ref.digest(request) == cpu["request_sha256"] and len(record.input_ids) == 359
                    and json.loads(json.dumps(asdict(record))) == cpu["records"][0]
                    and list(record.input_ids) == native_encoded["input_ids"] and native_encoded["id"] == "text-029"
                    and record.media is None, "exact text, IDs, spans or options differ")
        question = record.questions[1]
        ref.require(question.question_id == "日本1" and list(question.option_ids) == ["true", "false"]
                    and list(question.question_span) == [192, 194]
                    and [list(v) for v in question.option_spans] == [[205, 223], [229, 247]], "diagnostic coordinate differs")
        case = {"id": "text-029", "request_sha256": ref.digest(request), "record": record}
        result.update(input_ids=list(record.input_ids), record=asdict(record), request_sha256=case["request_sha256"],
                      native_http={}, native_logits={}, saved_A_logits=cpu["logits"])
        for mode in ("staged", "fused-fields"):
            rows = [json.loads(v) for v in (ref.NATIVE / mode / "rows.jsonl").read_text().splitlines() if v]
            rows = sorted((v for v in rows if v["request_sha256"] == "sha256:" + case["request_sha256"]), key=lambda v: v["question_index"])
            ref.require(len(rows) == 3, "native head coverage differs")
            for index, (row, q) in enumerate(zip(rows, record.questions)):
                ref.require(row["question_index"] == index and row["question"] == q.question_id
                            and row["rows"][2:] == ["option:" + v for v in q.option_ids], "native head coordinates differ")
            values = [row["logits"] for row in rows]
            result["native_http"][mode] = ref.native_response(case, mode, values)
            result["native_logits"][mode] = values
        result["fixtures"] = {"attention": attention_fixtures(torch, modeling), "delta": delta_fixtures(torch, modeling)}
        reader = ref.StreamingGguf()
        ref.require(reader.state == cpu["gguf_file_state"], "GGUF snapshot changed")
        result.update(gguf_state=reader.state, gguf_sha256_from_prior_verification=cpu["gguf_sha256_from_prior_verification"],
                      independent_decoder=reader.decoder_identity)
        head = official.JointSchemaHead(**json.loads((ref.HF / "joint_head_config.json").read_text()))
        head.load_state_dict(load_file(str(ref.HF / "joint_head.safetensors"), device="cpu"), strict=True)
        head = head.float().eval()
        head_before = parameter_hashes(torch, head)
        # finish_head applies backbone output RMSNorm exactly once to pre-norm layer-63 rows.
        a_pre = cpu_layer(63)
        a_final, a_head = ref.finish_head(torch, official, reader, config, case, a_pre, head)
        ref.require(a_head["logits"] == cpu["logits"] and a_head["hidden_sha256"] == cpu["hidden_sha256"]
                    and a_head["lexical_sha256"] == cpu["lexical_sha256"], "saved A complete head replay differs")
        result["A_head_replay_exact"] = True
        del a_pre, a_final
        embedding = torch.from_numpy(reader.rows("token_embd.weight", list(record.input_ids))).unsqueeze(0)
        shape(embedding)
        ref.require(ref.digest(ref.cpu_bytes(torch, embedding)) == factorial["input_sha256"], "initial embedding differs")
        hidden = {name: embedding.clone() for name in MODES}
        del embedding
        adapter, adapter_identity, source = ref.make_negative_a_adapter(modeling)
        (args.output_dir / "negative_a_forward.py").write_text(source)
        result["negative_a_adapter"] = adapter_identity
        position_ids = torch.arange(359).view(1, -1)
        rotary = modeling.Qwen3_5TextRotaryEmbedding(config, device=torch.device("cpu"))
        with torch.inference_mode():
            rope = rotary(hidden[MODES[0]], position_ids[None, ...].expand(3, 1, -1))
            mask = modeling.create_causal_mask(config=config, inputs_embeds=hidden[MODES[0]],
                attention_mask=torch.ones((1, 359), dtype=torch.long), past_key_values=None, position_ids=position_ids)
        original_eager = modeling.eager_attention_forward
        raw_output_bytes = 0
        for name in MODES:
            (args.output_dir / name).mkdir()
        for index in range(64):
            started = time.monotonic()
            reader.check_state()
            with torch.device("meta"):
                layer = modeling.Qwen3_5DecoderLayer(config, index)
            arrays, negative = ref.layer_arrays(reader, index)
            ref.require(set(arrays) == set(layer.state_dict()), "layer parameters differ")
            layer.load_state_dict({k: torch.from_numpy(v) for k, v in arrays.items()}, strict=True, assign=True)
            if negative is not None:
                layer.linear_attn.register_buffer("gguf_negative_a", torch.from_numpy(negative))
                layer.linear_attn.forward = types.MethodType(adapter, layer.linear_attn)
            del arrays, negative
            layer.eval()
            names = []
            with torch.no_grad():
                for key, module in layer.named_modules():
                    if isinstance(module, torch.nn.Linear):
                        module.weight.copy_(module.weight.half().float())
                        names.append(key)
            ref.require(len(names) == (7 if index % 4 == 3 else 8)
                        and all(bool(torch.isfinite(v).all()) for v in layer.parameters()), "rounded weights invalid")
            weights = ref.layer_hashes(torch, layer, "cpu")
            parameter_versions = ref.versions(layer)
            originals = {}
            linear_calls = {}
            online_calls = 0

            def wrap(key, module):
                original = module.forward

                def forward(self, value):
                    linear_calls[key] = linear_calls.get(key, 0) + 1
                    return original(value.half().float())

                originals[key] = (module, original)
                module.forward = types.MethodType(forward, module)

            def attention(module, query, key, value, attention_mask, scaling, dropout=0.0, **kwargs):
                nonlocal online_calls
                online_calls += 1
                ref.require(dropout == 0 and not module.training, "attention training or dropout differs")
                return online_attention(torch, modeling, module, query, key, value, attention_mask,
                                        scaling, lambda *a, **k: None)

            layer_result = {"layer": index, "kind": config.layer_types[index], "weight_hashes": weights,
                            "rounded_linear_names": names, "load_round_hash_seconds": time.monotonic() - started, "variants": {}}
            try:
                for key, module in layer.named_modules():
                    if isinstance(module, torch.nn.Linear):
                        wrap(key, module)
                for name in MODES:
                    linear_calls.clear()
                    online_calls = 0
                    before = ref.cpu_bytes(torch, hidden[name])
                    modeling.eager_attention_forward = attention if name == MODES[1] else original_eager
                    tick = time.monotonic()
                    with torch.inference_mode():
                        output = layer(hidden[name], position_embeddings=rope,
                            attention_mask=mask if index % 4 == 3 else None, position_ids=position_ids,
                            past_key_values=None, use_cache=False)
                    shape(output)
                    ref.require(ref.cpu_bytes(torch, hidden[name]) == before and ref.versions(layer) == parameter_versions,
                                "forward changed input or parameter storage/version")
                    ref.require(linear_calls == {key: 1 for key in names}
                                and online_calls == int(name == MODES[1] and index % 4 == 3), "operator call coverage differs")
                    raw = ref.cpu_bytes(torch, output)
                    if index == 0:
                        expected_first = factorial["variants"][3]["checkpoints"]["layer.output"]["raw_sha256"]
                        ref.require(ref.digest(raw) == expected_first, "layer-0 result differs from frozen WX control")
                    path = args.output_dir / name / f"layer-{index:02d}.safetensors"
                    save_file({"hidden": output.contiguous()}, str(path))
                    raw_output_bytes += len(raw)
                    ref.require(raw_output_bytes <= 2 * 1024**3 - 32 * 1024**2, "raw output cap exceeded")
                    layer_result["variants"][name] = {"file": str(path.relative_to(args.output_dir)),
                        "file_sha256": ref.sha(path), "hidden_sha256": ref.digest(raw), "shape": list(output.shape),
                        "bytes": len(raw), "forward_save_seconds": time.monotonic() - tick}
                    hidden[name] = output
                    del output, raw, before
                ref.require(ref.layer_hashes(torch, layer, "cpu") == weights, "forward changed weight bytes")
            finally:
                modeling.eager_attention_forward = original_eager
                for module, original in originals.values():
                    module.forward = original
                originals.clear()
            del layer, module, original
            gc.collect()
            a_rows, n_rows = cpu_layer(index), native_rows(f"l_out-{index}.f32")
            b_rows, c_rows = hidden[MODES[0]], hidden[MODES[1]]
            if index < 3:
                ref.require(ref.cpu_bytes(torch, b_rows) == ref.cpu_bytes(torch, c_rows), "controls diverged before first full attention")
            layer_result["comparisons"] = {}
            for label, lhs, rhs in (("B_minus_A", b_rows, a_rows), ("C_minus_A", c_rows, a_rows),
                    ("C_minus_B", c_rows, b_rows), ("B_minus_native", b_rows, n_rows), ("C_minus_native", c_rows, n_rows)):
                layer_result["comparisons"][label] = {"whole": ref.row_errors(lhs, rhs),
                    "question": ref.row_errors(lhs[:, 192:194], rhs[:, 192:194]),
                    "true_option": ref.row_errors(lhs[:, 205:223], rhs[:, 205:223]),
                    "false_option": ref.row_errors(lhs[:, 229:247], rhs[:, 229:247])}
            result["layers"].append(layer_result)
            ref.write(receipt, result)
            print(json.dumps({"layer": index, "seconds": time.monotonic() - started,
                              "B_native_relative_l2": layer_result["comparisons"]["B_minus_native"]["whole"]["relative_l2"],
                              "C_native_relative_l2": layer_result["comparisons"]["C_minus_native"]["whole"]["relative_l2"]}), flush=True)
            del a_rows, n_rows, b_rows, c_rows, lhs, rhs
            reader.check_state()

        result["heads"] = {}
        reference_final = load_file(str(HERE / "cpu-1/results/final-hidden.safetensors"), device="cpu")["hidden"]
        shape(reference_final)
        ref.require(ref.digest(ref.cpu_bytes(torch, reference_final)) == cpu["hidden_sha256"], "saved final rows changed")
        native_final = native_rows("result_norm.f32")
        for name in MODES:
            normalized, values = ref.finish_head(torch, official, reader, config, case, hidden[name], head)
            shape(normalized)
            ref.require(values["lexical_sha256"] == cpu["lexical_sha256"], "head lexical rows changed")
            tokens = values["option_token_ids"]
            lexical = torch.from_numpy(reader.rows("output.weight", tokens))
            ref.require(ref.digest(ref.cpu_bytes(torch, lexical)) == values["lexical_sha256"], "lexical export changed")
            remap = {token: i for i, token in enumerate(tokens)}
            remapped = [remap.get(token, 0) for token in record.input_ids]
            path = args.output_dir / name / "head-inputs.safetensors"
            save_file({"hidden": normalized.contiguous(), "lexical": lexical.contiguous(),
                "input_ids": torch.tensor([list(record.input_ids)]), "lexical_token_ids": torch.tensor(tokens),
                "remapped_input_ids": torch.tensor([remapped]), "attention_mask": torch.ones((1, 359), dtype=torch.long)}, str(path))
            values.update(head_inputs_file=str(path.relative_to(args.output_dir)), head_inputs_file_sha256=ref.sha(path),
                final_vs_A=ref.row_errors(normalized, reference_final), final_vs_native=ref.row_errors(normalized, native_final),
                logits_vs_A=ref.compare_logits(values["logits"], cpu["logits"]),
                logits_vs_native={mode: ref.compare_logits(values["logits"], logits) for mode, logits in result["native_logits"].items()})
            result["heads"][name] = values
            hidden[name] = normalized
            del normalized, lexical
        result["final_C_minus_B"] = {"hidden": ref.row_errors(hidden[MODES[1]], hidden[MODES[0]]),
            "logits": ref.compare_logits(result["heads"][MODES[1]]["logits"], result["heads"][MODES[0]]["logits"])}
        ref.require(parameter_hashes(torch, head) == head_before, "head weights changed")
        ref.require(len(result["layers"]) == 64 and len(result["heads"]) == 2, "full control coverage incomplete")
        reader.check_state()
        result.update(status="complete", completed=True, payload_read_bytes=reader.payload_read_bytes,
                      raw_layer_output_bytes=raw_output_bytes, unchanged_head_weights=True, original_artifact_unchanged=True)
    except BaseException as error:
        result.update(status="failed", error_type=type(error).__name__, error=str(error))
        raise
    finally:
        if original_eager is not None:
            modeling.eager_attention_forward = original_eager
        if reader is not None:
            reader.close()
        ref.write(receipt, result)


if __name__ == "__main__":
    main()
