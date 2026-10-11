#!/usr/bin/env python3
"""Separate half boundaries and Delta reduction order on identical layer-0 inputs."""
import argparse
import gc
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
from layer0_math import delta_fixtures

VARIANTS = (
    ("A_f32_chunk", False, False, False),
    ("W_weight_half_chunk", True, False, False),
    ("X_activation_half_chunk", False, True, False),
    ("WX_both_half_chunk", True, True, False),
    ("R_f32_recurrent", False, False, True),
    ("WXR_both_half_recurrent", True, True, True),
)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    leases = set(os.environ.get("OPENAGENTS_LEASES", "").split(","))
    ref.require({"memory", "disk"} <= leases and not {"gpu", "build", "quiet"} & leases
                and os.environ.get("CLEF_LAYER0_GUARDED") == "1", "CPU-only guarded replay required")
    ref.require(not args.output_dir.exists(), "preserve previous output")
    args.output_dir.mkdir(parents=True)
    result = {
        "schema": "clef.text029-layer0-factorial.v1", "status": "running", "layer": 0,
        "gpu_executed": False, "head_executed": False, "artifact_weight_files_modified": False,
        "variants": [], "acceptance_scope": "Diagnostic only; unchanged 40/104 all-top and 0.02 probability gate.",
        "precision_scope": "Round only nn.Linear weights and/or their inputs through f16. Conv weights, norms, residuals, Delta preparation and state remain f32.",
        "recurrent_scope": "R variants use the independent official recurrent operator, not a port of the Metal kernel. Native uses a different dot/FMA/output-update order.",
        "limitations": "Neither control reproduces Metal GEMM/RMSNorm reductions, transcendental functions, residual-in-GEMM accumulation or fused multiply-add bitwise.",
    }
    receipt = args.output_dir / "receipt.json"
    ref.write(receipt, result)
    reader = None
    try:
        import numpy as np
        import torch
        import transformers
        from safetensors.torch import load_file, save_file
        from transformers.models.qwen3_5 import modeling_qwen3_5 as modeling

        torch.set_num_threads(2)
        torch.set_num_interop_threads(1)
        torch.set_default_device("cpu")
        ref.require(str(torch.__version__) == "2.9.1" and transformers.__version__ == "5.10.2"
                    and ref.sha(Path(modeling.__file__)) == ref.MODELING_SHA, "official operators differ")
        ref.require(modeling.FusedRMSNormGated is None and modeling.causal_conv1d_fn is None
                    and modeling.chunk_gated_delta_rule is None and modeling.fused_recurrent_gated_delta_rule is None,
                    "unexpected optional kernels")
        for name, digest in ref.PINS.items():
            ref.require(ref.sha(ref.CPU / name) == digest, "independent importer changed")
        config = modeling.Qwen3_5TextConfig.from_dict(json.loads((ref.HF / "config.json").read_text())["text_config"])
        config._attn_implementation = "eager"
        config.use_cache = False
        config.dtype = torch.float32
        ref.require(config.num_hidden_layers == 64 and config.hidden_size == 5120
                    and config.intermediate_size == 17408 and config.layer_types[0] == "linear_attention",
                    "layer-0 shape or kind differs")
        cpu = json.loads((HERE / "cpu-1/results/receipt.json").read_text())
        native = json.loads((HERE / "native-1/results/receipt.json").read_text())
        comparison = json.loads((HERE / "comparison-1/results/receipt.json").read_text())
        ref.require(comparison["observer_consistency_screen"]["passed"], "observer equivalence differs")
        ids = cpu["input_ids"]
        encoded = json.loads((RUN / "encoder-source-run-1/native-encoded.json").read_text())["cases"][29]
        ref.require(len(ids) == 359 and encoded["id"] == "text-029" and encoded["input_ids"] == ids,
                    "exact token IDs differ")
        q = cpu["records"][0]["questions"][1]
        ref.require(q["question_id"] == "日本1" and q["option_ids"] == ["true", "false"]
                    and q["question_span"] == [192, 194] and q["option_spans"] == [[205, 223], [229, 247]],
                    "diagnostic coordinates differ")
        row = cpu["layers"][0]
        cpu_file = HERE / "cpu-1/results" / row["file"]
        ref.require(ref.sha(cpu_file) == row["file_sha256"], "CPU layer file changed")
        expected = load_file(str(cpu_file), device="cpu")["hidden"]
        ref.require(ref.digest(ref.cpu_bytes(torch, expected)) == row["hidden_sha256"], "CPU layer bytes differ")
        native_file = HERE / "native-1/results/rows/l_out-0.f32"
        entry = next(v for v in native["files"] if v["name"] == native_file.name)
        ref.require(ref.sha(native_file) == entry["sha256"], "native layer file changed")
        target = torch.from_numpy(np.fromfile(native_file, dtype="<f4").copy().reshape(1, 359, 5120))
        for value in (expected, target):
            ref.require(tuple(value.shape) == (1, 359, 5120) and value.dtype == torch.float32
                        and bool(torch.isfinite(value).all()), "saved output shape or values differ")
        reader = ref.StreamingGguf()
        ref.require(reader.state == cpu["gguf_file_state"], "GGUF snapshot changed")
        hidden = torch.from_numpy(reader.rows("token_embd.weight", ids)).unsqueeze(0)
        ref.require(tuple(hidden.shape) == (1, 359, 5120) and bool(torch.isfinite(hidden).all()), "embedding differs")
        original_input = ref.cpu_bytes(torch, hidden)
        result.update(input_ids=ids, request_sha256=cpu["request_sha256"], input_sha256=ref.digest(original_input),
                      input_identity_limit="Native input was reconstructed from exact IDs and the same verified GGUF embedding decoder; it was not independently captured before layer 0.",
                      gguf_state=reader.state, gguf_sha256_from_prior_verification=cpu["gguf_sha256_from_prior_verification"])
        result["arithmetic_fixtures"] = delta_fixtures(torch, modeling)
        adapter, adapter_identity, adapter_source = ref.make_negative_a_adapter(modeling)
        (args.output_dir / "negative_a_forward.py").write_text(adapter_source)
        result["negative_a_adapter"] = adapter_identity
        position_ids = torch.arange(359).view(1, -1)
        rotary = modeling.Qwen3_5TextRotaryEmbedding(config, device=torch.device("cpu"))
        with torch.inference_mode():
            rope = rotary(hidden, position_ids[None, ...].expand(3, 1, -1))
        total_checkpoint_bytes = 0
        for name, weight_half, activation_half, recurrent in VARIANTS:
            started = time.monotonic()
            reader.check_state()
            with torch.device("meta"):
                layer = modeling.Qwen3_5DecoderLayer(config, 0)
            arrays, negative = ref.layer_arrays(reader, 0)
            ref.require(negative is not None and set(arrays) == set(layer.state_dict()), "hybrid parameters differ")
            layer.load_state_dict({k: torch.from_numpy(v) for k, v in arrays.items()}, strict=True, assign=True)
            layer.linear_attn.register_buffer("gguf_negative_a", torch.from_numpy(negative))
            layer.linear_attn.forward = types.MethodType(adapter, layer.linear_attn)
            del arrays, negative
            layer.eval()
            unrounded = ref.layer_hashes(torch, layer, "cpu")
            if result["variants"]:
                ref.require(unrounded == result["parameter_identity_before"], "fresh original weights changed")
            else:
                result["parameter_identity_before"] = unrounded
            if weight_half:
                with torch.no_grad():
                    for module in layer.modules():
                        if isinstance(module, torch.nn.Linear):
                            module.weight.copy_(module.weight.half().float())
            ref.require(all(bool(torch.isfinite(p).all()) for p in layer.parameters()), "nonfinite parameter")
            weight_hashes = ref.layer_hashes(torch, layer, "cpu")
            points, handles, originals = {}, [], {}
            directory = args.output_dir / name
            directory.mkdir()
            calls = 0

            def capture(key, value):
                nonlocal total_checkpoint_bytes
                ref.require(key not in points and value.dtype == torch.float32 and value.device.type == "cpu"
                            and bool(torch.isfinite(value).all()), "checkpoint repeat/device/dtype/finiteness: " + key)
                raw = ref.cpu_bytes(torch, value)
                total_checkpoint_bytes += len(raw)
                ref.require(total_checkpoint_bytes <= 2 * 1024**3 - 8 * 1024**2, "checkpoint byte cap exceeded")
                path = directory / (key + ".safetensors")
                save_file({"value": value.detach().contiguous()}, str(path))
                points[key] = {"file": str(path.relative_to(args.output_dir)), "shape": list(value.shape),
                               "bytes": len(raw), "raw_sha256": ref.digest(raw), "file_sha256": ref.sha(path)}

            def wrap_linear(key, module):
                original = module.forward

                def forward(self, value):
                    used = value.half().float() if activation_half else value
                    capture(key + ".input", used)
                    output = original(used)
                    capture(key + ".output", output)
                    return output

                originals[key] = (module, original)
                module.forward = types.MethodType(forward, module)

            original_delta = layer.linear_attn.chunk_gated_delta_rule

            def delta(query, key, value, g, beta, initial_state=None, output_final_state=False,
                      use_qk_l2norm_in_kernel=False, **kwargs):
                nonlocal calls
                calls += 1
                ref.require(initial_state is None and not output_final_state and use_qk_l2norm_in_kernel
                            and all(k == "cu_seqlens" and v is None for k, v in kwargs.items()),
                            "unexpected cached or packed Delta call")
                inputs = {"query": query, "key": key, "value": value, "g": g, "beta": beta}
                original_hashes = {k: ref.digest(ref.cpu_bytes(torch, v)) for k, v in inputs.items()}
                for key_name, tensor in inputs.items():
                    capture("delta." + key_name, tensor)
                capture("delta.q_normalized_scaled", modeling.l2norm(query, dim=-1, eps=1e-6) / (128**0.5))
                capture("delta.k_normalized", modeling.l2norm(key, dim=-1, eps=1e-6))
                operator = modeling.torch_recurrent_gated_delta_rule if recurrent else original_delta
                output, state = operator(query, key, value, g=g, beta=beta, initial_state=None,
                                         output_final_state=True, use_qk_l2norm_in_kernel=True)
                capture("delta.output", output)
                capture("delta.final_state", state)
                ref.require(all(ref.digest(ref.cpu_bytes(torch, v)) == original_hashes[k]
                                for k, v in inputs.items()), "Delta operator mutated input")
                return output, None

            def post_norm(module, inputs, output):
                capture("mixer.residual", inputs[0])
                capture("ffn.norm", output)

            try:
                for key, module in layer.named_modules():
                    if isinstance(module, torch.nn.Linear):
                        wrap_linear(key, module)
                ref.require(len(originals) == 8, "expected eight linear projections")
                handles = [
                    layer.input_layernorm.register_forward_hook(lambda m, a, o: capture("mixer.norm", o)),
                    layer.post_attention_layernorm.register_forward_hook(post_norm),
                    layer.linear_attn.register_forward_hook(lambda m, a, o: capture("mixer.output", o)),
                    layer.mlp.register_forward_hook(lambda m, a, o: capture("ffn.output", o)),
                ]
                layer.linear_attn.chunk_gated_delta_rule = delta
                with torch.inference_mode():
                    output = layer(hidden, position_embeddings=rope, attention_mask=None,
                                   position_ids=position_ids, past_key_values=None, use_cache=False)
                capture("layer.output", output)
                ref.require(calls == 1 and len(points) == 31, "checkpoint or Delta call coverage differs")
                ref.require(ref.cpu_bytes(torch, hidden) == original_input, "input mutated")
                ref.require(ref.layer_hashes(torch, layer, "cpu") == weight_hashes, "parameter mutated")
                exact = ref.cpu_bytes(torch, output) == ref.cpu_bytes(torch, expected) if name == "A_f32_chunk" else None
                if exact is not None:
                    ref.require(exact, "baseline does not reproduce saved CPU layer 0 bitwise")
                result["variants"].append({"name": name, "weight_half": weight_half,
                    "activation_half": activation_half, "delta_operator": "official_recurrent" if recurrent else "official_chunk64",
                    "seconds": time.monotonic() - started, "baseline_exact": exact,
                    "input_sha256": ref.digest(original_input), "parameter_hashes": weight_hashes, "checkpoints": points,
                    "output_vs_native": ref.row_errors(output, target), "output_vs_cpu": ref.row_errors(output, expected),
                    "false_span_vs_native": ref.row_errors(output[:, 229:247], target[:, 229:247]),
                    "false_span_vs_cpu": ref.row_errors(output[:, 229:247], expected[:, 229:247])})
                ref.write(receipt, result)
            finally:
                layer.linear_attn.chunk_gated_delta_rule = original_delta
                for handle in handles:
                    handle.remove()
                for module, original in originals.values():
                    module.forward = original
                originals.clear()
                handles.clear()
            del layer, output, original_delta, module, original
            gc.collect()
            reader.check_state()

        result["checkpoint_comparisons"] = []
        for left, right in ((1, 0), (2, 0), (3, 0), (3, 1), (3, 2), (4, 0), (5, 3), (5, 4)):
            lhs, rhs = result["variants"][left], result["variants"][right]
            for key in sorted(lhs["checkpoints"]):
                lp, rp = lhs["checkpoints"][key], rhs["checkpoints"][key]
                ref.require(ref.sha(args.output_dir / lp["file"]) == lp["file_sha256"]
                            and ref.sha(args.output_dir / rp["file"]) == rp["file_sha256"], "checkpoint changed")
                lv = load_file(str(args.output_dir / lp["file"]), device="cpu")["value"]
                rv = load_file(str(args.output_dir / rp["file"]), device="cpu")["value"]
                result["checkpoint_comparisons"].append({"left": lhs["name"], "right": rhs["name"],
                    "checkpoint": key, "metrics": ref.row_errors(lv, rv)})
                del lv, rv
        ref.require(len(result["variants"]) == 6 and len(result["checkpoint_comparisons"]) == 248,
                    "factorial comparison coverage differs")
        result.update(status="complete", completed=True, original_artifact_unchanged=True,
                      payload_read_bytes=reader.payload_read_bytes, checkpoint_bytes=total_checkpoint_bytes)
    except BaseException as error:
        result.update(status="failed", error_type=type(error).__name__, error=str(error))
        raise
    finally:
        if reader is not None:
            reader.close()
        ref.write(receipt, result)


if __name__ == "__main__":
    main()
