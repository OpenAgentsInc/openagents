# Firm reason: 2 (independent Pillow decoding/comparison of actual GPU captures against frozen reference frames).
"""Compare fixed-epoch 1080p orbit captures; licensed pixels stay caller-local."""
import argparse
import json
from pathlib import Path
from PIL import Image, ImageChops, ImageStat

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("before", type=Path)
parser.add_argument("after", type=Path)
parser.add_argument("--count", type=int, default=217)
args = parser.parse_args()
if args.count < 1 or args.count > 1000:
    parser.error("count must be 1..1000")
means, histogram, identical = [], [0] * 256, 0
for frame in range(args.count):
    name = f"frame-{frame:03d}.png"
    with Image.open(args.before / name) as original, Image.open(args.after / name) as candidate:
        if original.size != (1920, 1080) or candidate.size != original.size:
            raise ValueError(f"wrong raster: {name}")
        difference = ImageChops.difference(original.convert("RGB"), candidate.convert("RGB"))
    means.append(sum(ImageStat.Stat(difference).mean) / 3)
    identical += difference.getbbox() is None
    channels = difference.histogram()
    for value in range(256):
        histogram[value] += sum(channels[value::256])
total, cumulative, p99 = sum(histogram), 0, 0
for value, count in enumerate(histogram):
    cumulative += count
    if cumulative >= total * 0.99:
        p99 = value
        break
result = {
    "frames": args.count, "raster": [1920, 1080], "identical": identical,
    "mean_abs_rgb8": sum(means) / len(means), "worst_frame_mean_rgb8": max(means),
    "p99_rgb8": p99, "channels_over_2_fraction": sum(histogram[3:]) / total,
    "max_rgb8": max(value for value, count in enumerate(histogram) if count),
}
# Varying interpolation can move rare quantized sky/noise boundaries. Preserve
# the full image, report the extreme tail, and refuse a broad or systematic change.
result["passed"] = (
    result["mean_abs_rgb8"] <= 0.25 and result["worst_frame_mean_rgb8"] <= 0.5
    and p99 <= 2 and result["channels_over_2_fraction"] <= 0.0001
)
print(json.dumps(result, indent=2))
raise SystemExit(0 if result["passed"] else 1)
