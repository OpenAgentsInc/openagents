#!/usr/bin/env python3
"""Firm reason2: resident original-weight framework oracle, opt-in tooling only.

Official installed Transformers backbone + independently authored source head.
All learned arithmetic is MPS/F32. Coax owns shared observation preparation,
decoder, supervisor, controller and scene effects. This is not NVIDIA's BF16
wrapper, seeded reset distribution or published benchmark.
"""
import argparse
import ctypes as C
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import time
import traceback
import numpy as np
import torch
import transformers
import diffusers
from transformers import Qwen3VLConfig

F32 = C.POINTER(C.c_float)
I64 = C.POINTER(C.c_int64)
U8 = C.POINTER(C.c_uint8)
INFER = C.CFUNCTYPE(C.c_int, C.c_void_p, F32, I64, C.c_int64, F32, F32, F32)

class Timing(C.Structure):
    _fields_ = [(k, C.c_double) for k in ("preparation_seconds", "vision_gpu_seconds", "text_gpu_seconds", "action_gpu_seconds", "total_seconds")] + [("tokens", C.c_int64), ("launches", C.c_int64)]

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def array(pointer, shape):
    return np.ctypeslib.as_array(pointer, shape=shape)

class ResidentReference:
    def __init__(self, weights, cosmos, output):
        assert torch.__version__.split("+")[0] == "2.9.0" and transformers.__version__ == "4.57.3" and diffusers.__version__ == "0.35.1"
        assert torch.backends.mps.is_available()
        assert os.environ.get("PYTORCH_ENABLE_MPS_FALLBACK", "0") == "0"
        assert os.environ.get("PYTORCH_MPS_FAST_MATH", "0") == "0"
        source = Path(__file__).with_name("groot_vla_reference.py")
        spec = importlib.util.spec_from_file_location("panda_independent_original_head", source)
        self.reference = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.reference)
        config_path = cosmos / "config.json"
        assert sha(config_path) == "bec4b3d446efa05807365c9e1cec03ac590836879d02f3a6da879971154bdd3b"
        config = Qwen3VLConfig.from_dict(json.loads(config_path.read_text()))
        config.text_config.num_hidden_layers = 16
        config._attn_implementation = config.text_config._attn_implementation = config.vision_config._attn_implementation = "sdpa"
        started = time.monotonic()
        self.model, tensors = self.reference.load_weights(weights, config)
        assert all(p.device.type == "mps" and p.dtype == torch.float32 for p in self.model.parameters())
        assert len(tensors) == 537 and all(p.device.type == "mps" and p.dtype == torch.float32 for p in tensors.values())
        self.head = self.reference.ActionHead(tensors, self.require_mps)
        self.grid = torch.tensor([[1, 16, 16], [1, 16, 16]], device="mps")
        self.ids = self.attention = self.noise = None
        self.ids_bytes = self.noise_bytes = None
        self.output = output
        self.requests = output / "reference-requests"
        self.requests.mkdir()
        self.journal = (output / "requests.jsonl").open("x")
        self.calls, self.error = 0, None
        torch.mps.synchronize()
        self.load_seconds = time.monotonic() - started

    @staticmethod
    def require_mps(name, value):
        assert value.device.type == "mps", name
        return value

    def infer(self, user, pixels_ptr, ids_ptr, tokens, state_ptr, noise_ptr, out_ptr):
        try:
            assert tokens == 152
            pixels = array(pixels_ptr, (512 * 1536,)).reshape(512, 1536)
            ids = array(ids_ptr, (tokens,))
            state = array(state_ptr, (132,)).reshape(1, 1, 132)
            noise = array(noise_ptr, (5280,)).reshape(1, 40, 132)
            assert all(np.isfinite(x).all() for x in (pixels, state, noise))
            ids_bytes, noise_bytes = ids.tobytes(), noise.tobytes()
            if self.ids is None:
                self.ids_bytes, self.noise_bytes = ids_bytes, noise_bytes
                self.ids = torch.from_numpy(ids.copy()[None]).to("mps")
                self.attention = torch.ones_like(self.ids)
                self.noise = torch.from_numpy(noise.copy()).to("mps")
            assert ids_bytes == self.ids_bytes and noise_bytes == self.noise_bytes
            started = time.monotonic()
            with torch.inference_mode():
                result = self.model(input_ids=self.ids, attention_mask=self.attention,
                                    pixel_values=torch.from_numpy(pixels).to("mps"),
                                    image_grid_thw=self.grid, output_hidden_states=False,
                                    use_cache=False)
                features = self.require_mps("backbone", result.last_hidden_state)
                normalized = self.head(features, torch.from_numpy(state).to("mps"), self.noise, self.ids)
                self.require_mps("normalized-actions", normalized)
                value = normalized.detach().float().cpu().numpy().reshape(5280).copy()
            torch.mps.synchronize()
            elapsed = time.monotonic() - started
            assert np.isfinite(value).all()
            # Private prepared-observation capsules support later SAME-input
            # diagnostics; no hidden-layer captures or public payloads.
            name = f"request{self.calls:04d}.npz"
            filename = self.requests / name
            np.savez(filename, pixels=pixels, ids=ids, state=state, noise=noise, normalized_actions=value)
            entry = {"request": self.calls, "file": name, "bytes": filename.stat().st_size, "sha256": sha(filename), "model_wall_seconds": elapsed,
                     "prepared_sha256": {k: hashlib.sha256(v.tobytes()).hexdigest() for k, v in (("pixels", pixels), ("ids", ids), ("state", state), ("noise", noise))},
                     "normalized_actions_sha256": hashlib.sha256(value.tobytes()).hexdigest()}
            self.journal.write(json.dumps(entry, sort_keys=True) + "\n")
            self.journal.flush()
            np.copyto(array(out_ptr, (5280,)), value)
            self.calls += 1
            print(f"reference request{self.calls}: modelMPS wall{elapsed:.6f}s", flush=True)
            return 1
        except BaseException:
            self.error = traceback.format_exc()
            print(self.error, flush=True)
            return 0

def load_bridge(path):
    lib = C.CDLL(str(path))
    lib.panda_reference_open.argtypes = [C.c_char_p, C.c_char_p, INFER, C.c_void_p, C.c_char_p, C.c_size_t]
    lib.panda_reference_open.restype = C.c_void_p
    lib.panda_reference_forward.argtypes = [C.c_void_p, U8, U8, F32, F32, F32, C.POINTER(Timing)]
    lib.panda_reference_forward.restype = C.c_int
    lib.panda_reference_error.argtypes = [C.c_void_p]
    lib.panda_reference_error.restype = C.c_char_p
    lib.panda_reference_copy.argtypes = [C.c_void_p, C.c_char_p, C.c_void_p, C.c_size_t]
    lib.panda_reference_copy.restype = C.c_int
    lib.panda_reference_run.argtypes = [C.c_void_p, C.c_int, C.POINTER(C.c_char_p), C.c_double]
    lib.panda_reference_run.restype = C.c_int
    lib.panda_reference_close.argtypes = [C.c_void_p]
    return lib

def qualify(a, lib, context):
    source = json.loads((a.qualify / "record.json").read_text())
    assert source["schema"] == "psionic-groot-prepared-vla-reference-v2" and source["request_tokens"] == 152
    assert sha(a.qualify / "inputs.npz") == source["prepared_observation_sha256"]
    raw = np.load(a.raw_frame, allow_pickle=False)
    inputs = np.load(a.qualify / "inputs.npz", allow_pickle=False)
    first, wrist = [np.ascontiguousarray(raw[k], dtype=np.uint8) for k in ("agentview_rgb", "wrist_rgb")]
    state = np.ascontiguousarray(raw["state8"], dtype=np.float32)
    noise = np.ascontiguousarray(inputs["noise"].reshape(5280), dtype=np.float32)
    decoded, timing = np.full(112, np.float32(9999)), Timing()
    assert lib.panda_reference_forward(context, first.ctypes.data_as(U8), wrist.ctypes.data_as(U8), state.ctypes.data_as(F32), noise.ctypes.data_as(F32), decoded.ctypes.data_as(F32), C.byref(timing)), lib.panda_reference_error(context)
    for key, dtype in (("pixels", np.float32), ("ids", np.int64), ("state", np.float32), ("noise", np.float32)):
        actual = np.empty(inputs[key].size, dtype=dtype)
        assert lib.panda_reference_copy(context, key.encode(), actual.ctypes.data, actual.nbytes)
        assert actual.tobytes() == inputs[key].tobytes(), key
    expected_path = a.qualify / "normalized-actions.f32"
    item = next(x for x in source["outputs"] if x["name"] == "normalized-actions")
    assert sha(expected_path) == item["sha256"]
    expected = np.fromfile(expected_path, dtype="<f4")
    actual = np.empty(5280, dtype=np.float32)
    assert lib.panda_reference_copy(context, b"normalized-actions", actual.ctypes.data, actual.nbytes)
    error = np.abs(expected.astype(np.float64) - actual.astype(np.float64))
    budget = .002 + .002 * np.abs(expected.astype(np.float64))
    assert np.all(error <= budget)
    normalized_file = a.output / "normalized-actions.f32"
    normalized_file.write_bytes(actual.tobytes())
    (a.output / "decoded-actions.f32").write_bytes(decoded.tobytes())
    receipt = {"schema": "psionic-panda-resident-framework-integration-v1", "passed": True, "reference_capsule_record_sha256": sha(a.qualify / "record.json"), "prepared_inputs_exact": True, "request_tokens": 152, "values_compared": 5280, "max_abs_error": float(error.max()), "max_budget_fraction": float((error / budget).max()), "frozen_tolerance": {"atol": .002, "rtol": .002}, "coax_normalized_sha256": sha(normalized_file), "bridge_forward_wall_seconds": timing.total_seconds, "shared_Coax_decoder_used": True, "source_default_BF16_identity_claim": False}
    (a.output / "qualification.json").write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
    print(json.dumps(receipt, sort_keys=True), flush=True)

def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ("library", "weights", "cosmos", "pst", "noise", "output"):
        p.add_argument("--" + name, required=True, type=Path)
    p.add_argument("--pairs", type=Path)
    p.add_argument("--case", choices=("020", "030", "040"))
    p.add_argument("--ticks", type=int, default=720)
    p.add_argument("--qualify", type=Path)
    p.add_argument("--raw-frame", type=Path)
    a = p.parse_args()
    assert (a.qualify is not None) != (a.case is not None)
    assert 1 <= a.ticks <= 720
    assert sha(a.noise) == "59e012a91bd82498415471c884fd6bc7f0e2a5fa920ddc48bf79d1a9c1e899a0"
    a.output.mkdir(exist_ok=False)
    opened = time.monotonic()
    model = ResidentReference(a.weights, a.cosmos, a.output)
    lib = load_bridge(a.library)
    callback = INFER(model.infer)
    error = C.create_string_buffer(1024)
    instruction = b"pick up the book and place it in the back compartment of the caddy"
    context = lib.panda_reference_open(os.fsencode(a.pst), instruction, callback, None, error, len(error))
    assert context, error.value
    open_seconds = time.monotonic() - opened
    try:
        if a.qualify:
            assert a.raw_frame is not None
            qualify(a, lib, context)
        else:
            assert a.pairs is not None
            argv = [b"panda-reference", os.fsencode(a.weights), os.fsencode(a.pst), os.fsencode(a.noise), b"unused-no-vendor-Metal-provider", os.fsencode(a.output), instruction, str(a.ticks).encode(), b"0", os.fsencode(a.pairs / ("demo_" + a.case))]
            arguments = (C.c_char_p * len(argv))(*argv)
            assert lib.panda_reference_run(context, len(argv), arguments, open_seconds) == 0
    finally:
        lib.panda_reference_close(context)
        model.journal.close()
    metadata = {"schema": "psionic-panda-framework-provider-v1", "completed": True, "provider": "installed Transformers4.57.3 Qwen3VL + independently authored source-equation head", "profile": "originalBF16weights/F32MPSlearnedmath/sharedCoaxprep+decode+control/fixednoise169", "model_revision": model.reference.REVISION, "cosmos_revision": model.reference.COSMOS, "category": 2, "request_tokens": 152, "torch": torch.__version__, "transformers": transformers.__version__, "diffusers": diffusers.__version__, "learned_device": "mps", "learned_dtype": "float32", "cpu_model_fallback": False, "cloned_repository_executed": False, "model_calls": model.calls, "model_load_seconds": model.load_seconds, "context_open_seconds": open_seconds, "NVIDIA_BF16_wrapper_or_benchmark_claim": False, "posthoc_failed_case_subset": a.case, "request_capsules": "Private prepared observations + normalized outputs for same-input diagnosis; no hidden-layer captures", "requests_journal_sha256": sha(a.output / "requests.jsonl"), "gpu_timing_measured": False, "inference_wall_includes_private_request_archive": True, "bridge_library_sha256": sha(a.library), "source_sha256": {Path(__file__).name: sha(__file__), "groot_vla_reference.py": sha(Path(__file__).with_name("groot_vla_reference.py"))}}
    (a.output / "provider.json").write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n")

if __name__ == "__main__":
    main()
