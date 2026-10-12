"""Generate preprocessing-only Clef goldens; never load model weights."""

import argparse
import base64
import hashlib
import inspect
import json
from pathlib import Path
from types import SimpleNamespace

import numpy as np
import PIL
from PIL import Image
import torch
import torchvision
import transformers
from transformers import AutoProcessor
from transformers.image_utils import SizeDict, load_image_as_tensor
from transformers.models.qwen2_vl.image_processing_qwen2_vl import smart_resize
from transformers.models.qwen3_5.modeling_qwen3_5 import Qwen3_5Model


def sha(data):
    return hashlib.sha256(data).hexdigest()


class PositionReference:
    """Use the reference position functions without constructing any model."""
    config = SimpleNamespace(vision_config=SimpleNamespace(spatial_merge_size=2))
    get_vision_position_ids = Qwen3_5Model.get_vision_position_ids
    get_rope_index = Qwen3_5Model.get_rope_index


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--model-metadata", type=Path, required=True)
    args = parser.parse_args()
    torch.set_num_threads(1)
    processor = AutoProcessor.from_pretrained(args.model_metadata, local_files_only=True)
    image_processor = processor.image_processor
    assert type(image_processor).__name__ == "Qwen2VLImageProcessor"
    assert any(base.__name__ == "TorchvisionBackend" for base in type(image_processor).__mro__)
    output = Path(__file__).parent
    rows = []
    cases = [
        ("small", 23, 17, "PNG"),
        ("odd", 333, 271, "PNG"),
        ("ties_even_down", 272, 272, "PNG"),
        ("ties_even_up", 304, 304, "PNG"),
        ("identity", 512, 256, "PNG"),
        ("jpeg", 319, 287, "JPEG"),
        ("jpeg_oriented", 319, 287, "JPEG"),
        ("webp", 319, 287, "WEBP"),
    ]
    for name, width, height, fmt in cases:
        y, x = np.mgrid[:height, :width]
        rgb = np.stack([
            (x * 13 + y * 7 + (x // 11) * 19) % 256,
            (x * 3 + y * 17 + (y // 7) * 23) % 256,
            (x * 11 + y * 5 + ((x + y) // 13) * 31) % 256,
        ], axis=-1).astype(np.uint8)
        suffix = {"PNG": "png", "JPEG": "jpg", "WEBP": "webp"}[fmt]
        source = output / f"{name}.{suffix}"
        options = {"quality": 93, "subsampling": 0} if fmt == "JPEG" else {"lossless": True} if fmt == "WEBP" else {}
        if name == "jpeg_oriented":
            exif = Image.Exif()
            exif[274] = 6
            options["exif"] = exif
        Image.fromarray(rgb).save(source, format=fmt, **options)
        encoded = base64.b64encode(source.read_bytes()).decode("ascii")
        # Clef forwards base64 strings to this exact backend. Its byte decoder
        # does not apply EXIF, unlike the separate PIL load_image entry point.
        decoded_tensor = image_processor.fetch_images(encoded)
        assert decoded_tensor.dtype == torch.uint8 and decoded_tensor.shape[0] == 3
        decoded = Image.fromarray(decoded_tensor.permute(1, 2, 0).contiguous().numpy())
        if fmt == "JPEG":
            decoded.save(output / f"{name}.decoded.png")
        tensor = decoded_tensor.unsqueeze(0)
        resized_height, resized_width = smart_resize(
            decoded.height, decoded.width, factor=image_processor.patch_size * image_processor.merge_size,
            min_pixels=image_processor.size.shortest_edge, max_pixels=image_processor.size.longest_edge,
        )
        resized = image_processor.resize(
            image=tensor, size=SizeDict(height=resized_height, width=resized_width),
            resample=image_processor.resample,
        )
        resized_rgb = resized[0].permute(1, 2, 0).contiguous().numpy()
        Image.fromarray(resized_rgb).save(output / f"{name}.expected.png")
        result = processor(text=["<|vision_start|><|image_pad|><|vision_end|>\n"], images=[encoded], return_tensors="pt")
        assert result["image_grid_thw"][0].tolist() == [
            1, resized_height // image_processor.patch_size, resized_width // image_processor.patch_size
        ], f"{name}: byte-input grid disagrees with decoded/resize dimensions"
        from_resized = image_processor(images=[resized[0]], do_resize=False, return_tensors="pt")
        assert torch.equal(result["pixel_values"], from_resized["pixel_values"]), (
            f"{name}: byte-input preprocessing disagrees with its decoded/resize intermediates"
        )
        flat = result["pixel_values"].to(torch.float32).contiguous().numpy().astype("<f4")
        rows.append({
            "name": name, "file": source.name, "format": fmt,
            "source_sha256": sha(source.read_bytes()),
            "width": decoded.width, "height": decoded.height,
            "resized_width": int(resized.shape[-1]), "resized_height": int(resized.shape[-2]),
            "grid_thw": result["image_grid_thw"][0].tolist(),
            "decoded_rgb_sha256": sha(np.asarray(decoded).tobytes()),
            **({"decoded_file": f"{name}.decoded.png"} if fmt == "JPEG" else {}),
            "resized_rgb_sha256": sha(resized_rgb.tobytes()),
            "pixel_values_sha256": sha(flat.tobytes()),
            "media_input_ids": result["input_ids"][0].tolist(),
        })
    media = processor(
        text=["<|vision_start|><|image_pad|><|vision_end|>" * 2 + "\n"],
        images=[Image.open(output / "small.png").convert("RGB"), Image.open(output / "odd.png").convert("RGB")],
        return_tensors="pt",
    )
    input_ids = torch.cat([torch.tensor([[11, 12]]), media["input_ids"], torch.tensor([[21, 22]])], dim=1)
    token_types = torch.where(input_ids == 248056, 1, 0)
    positions, _ = PositionReference().get_rope_index(input_ids, token_types, image_grid_thw=media["image_grid_thw"])
    record = {
        "reference_entrypoint": "https://huggingface.co/Cloudflare/clef/blob/main/joint_schema_model.py",
        "loader": "AutoProcessor.from_pretrained(path), no backend override",
        "processor": f"{type(processor).__module__}.{type(processor).__name__}",
        "image_processor": f"{type(image_processor).__module__}.{type(image_processor).__name__}",
        "backend": "TorchvisionBackend, CPU uint8 bicubic antialias=True",
        "byte_decoder": "TorchvisionBackend.fetch_images(base64), decode_image(apply_exif_orientation=False)",
        "byte_decoder_source_sha256": sha(Path(inspect.getfile(load_image_as_tensor)).read_bytes()),
        "versions": {"transformers": transformers.__version__, "torch": torch.__version__,
                     "torchvision": torchvision.__version__, "pillow": PIL.__version__, "numpy": np.__version__},
        "processor_config_sha256": sha((args.model_metadata / "processor_config.json").read_bytes()),
        "image_processor_source_sha256": sha(Path(inspect.getfile(type(image_processor))).read_bytes()),
        "newline_input_ids": processor.tokenizer("\n", add_special_tokens=False).input_ids,
        "position_case": {"images": ["small", "odd"], "input_ids": input_ids[0].tolist(),
                          "positions": positions[:, 0].T.tolist()},
        "fixtures": rows,
    }
    (output / "reference.json").write_text(json.dumps(record, indent=2) + "\n")
    print(json.dumps(record, indent=2))


if __name__ == "__main__":
    main()
