"""Hash CPU-f32 parameter bytes without changing scalar shape metadata."""
import hashlib


def parameter_hashes(torch, module):
    values = {**dict(module.named_parameters()), **dict(module.named_buffers())}
    result = {}
    for name, value in values.items():
        if value.device.type != "cpu" or value.dtype != torch.float32:
            raise ValueError("expected a CPU-f32 parameter or buffer")
        # Flatten only the byte-view input; a scalar's recorded shape remains [].
        raw = value.detach().contiguous().reshape(-1).view(torch.uint8).numpy().tobytes()
        result[name] = {"shape": list(value.shape), "sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw)}
    return result


def self_test(torch):
    """Use only tiny local tensors; do not load model files or initialize a GPU."""
    module = torch.nn.Module()
    values = {
        "scalar": torch.tensor(1.5, dtype=torch.float32, device="cpu"),
        "negative_zero": torch.tensor(-0.0, dtype=torch.float32, device="cpu"),
        "vector": torch.tensor([1.0, -2.0, 0.125], dtype=torch.float32, device="cpu"),
        "noncontiguous": torch.arange(12, dtype=torch.float32, device="cpu").reshape(3, 4).transpose(0, 1),
        "empty": torch.empty(0, dtype=torch.float32, device="cpu"),
    }
    before = {name: value.numpy().tobytes() for name, value in values.items()}
    for name, value in values.items():
        module.register_parameter(name, torch.nn.Parameter(value, requires_grad=False))
    module.register_buffer("scalar_buffer", torch.tensor(0.5, dtype=torch.float32, device="cpu"))
    result = parameter_hashes(torch, module)
    for name, value in {**values, "scalar_buffer": module.scalar_buffer}.items():
        expected = value.detach().numpy().tobytes()
        if result[name] != {"shape": list(value.shape), "sha256": hashlib.sha256(expected).hexdigest(), "bytes": len(expected)}:
            raise ValueError("parameter byte-hash fixture differs: " + name)
    if result["scalar"]["shape"] != [] or result["scalar"]["bytes"] != 4:
        raise ValueError("scalar shape or byte count changed")
    if any(value.numpy().tobytes() != before[name] for name, value in values.items()):
        raise ValueError("byte hashing mutated its input")
    invalid = torch.nn.Module()
    invalid.register_buffer("wrong_dtype", torch.tensor(1.0, dtype=torch.float64, device="cpu"))
    try:
        parameter_hashes(torch, invalid)
    except ValueError:
        rejected = True
    else:
        rejected = False
    if not rejected:
        raise ValueError("unsupported parameter dtype was accepted")
    return {"passed": True, "fixtures": result, "wrong_dtype_rejected": True,
            "model_files_loaded": False, "gpu_executed": False}
