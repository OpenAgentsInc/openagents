#!/usr/bin/env python3
"""Bake the public-domain Earth, Moon, Milky Way, and star data for Lagrange 1.

Requires Python 3, NumPy, and Pillow. The script downloads pinned NASA and CDS
source files to a cache directory outside the repository, checks each file
against its pinned SHA-256, and writes the baked outputs to
crates/verse/assets/lagrange/. Rerunning it with the same sources and library
versions reproduces every output byte for byte.

Usage:
    python3 scripts/bake-lagrange-sky.py [--cache ~/.cache/lagrange-sky] [--out DIR]

All image outputs use the equirectangular (plate carree) projection. Pixel
column x of an image W pixels wide covers the longitude interval
[-180 + 360*x/W, -180 + 360*(x+1)/W] degrees, and pixel row y of an image H
pixels tall covers the latitude interval [90 - 180*(y+1)/H, 90 - 180*y/H]
degrees. milky_way.png follows the source sky-map convention instead; see
PROVENANCE.md.
"""

import argparse
import gzip
import hashlib
import math
from pathlib import Path
import struct
import sys
import urllib.request
import zlib

import numpy as np
from PIL import Image


Image.MAX_IMAGE_PIXELS = None

EO = "https://eoimages.gsfc.nasa.gov/images/imagerecords"
SVS = "https://svs.gsfc.nasa.gov/vis/a000000"
SOURCES = {
    "world.topo.bathy.200407.3x5400x2700.jpg": (
        f"{EO}/73000/73751/world.topo.bathy.200407.3x5400x2700.jpg",
        "4f4240673a3a1b173d61b92ca4b07bac5fd17059ea5f725ba6da5a9c5386b7ba",
    ),
    "cloud_combined_8192.tif": (
        f"{EO}/57000/57747/cloud_combined_8192.tif",
        "d137775d8966ab8d443fd5126dc6e7ad72072bc1ed50555c5818d221735daf0f",
    ),
    "gebco_08_rev_bath_21600x10800.png": (
        f"{EO}/73000/73963/gebco_08_rev_bath_21600x10800.png",
        "b3a67076fccfbee73ebe437fd66a8ed4969fbc5ce949033b50675d43f379e403",
    ),
    "land_ocean_ice_8192.png": (
        f"{EO}/57000/57730/land_ocean_ice_8192.png",
        "aaddcd967a9f09fb2d7ef50ff452bebfcf10192c520465d5eb1ad8446c716e98",
    ),
    "lroc_color_16bit_srgb_4k.tif": (
        f"{SVS}/a004700/a004720/lroc_color_16bit_srgb_4k.tif",
        "9731fa8af425b6c2f88f277ecca82bf8c603f3743894f64ed7b25c5bfefa22ff",
    ),
    "ldem_4.tif": (
        f"{SVS}/a004700/a004720/ldem_4.tif",
        "330afa2556a86fd05ac6ba2f912f246600fdade35de2a0d90593d50d07b01b65",
    ),
    "milkyway_2020_4k.exr": (
        f"{SVS}/a004800/a004851/milkyway_2020_4k.exr",
        "2eb802d6e68d170b410f766c7fec07f7518619f6b6708fdc81e9302d93e74fdb",
    ),
    "catalog.gz": (
        "https://cdsarc.cds.unistra.fr/ftp/V/50/catalog.gz",
        "3dc44b1e90be8fbe5bcc7656032560f51275f985c7e3f783c9028e1838ec7bed",
    ),
}

# Milky Way exposure: output = sRGB(clamp(linear * MILKY_WAY_SCALE, 0, 1)).
# A shader recovers source radiance as srgb_to_linear(texel) / MILKY_WAY_SCALE.
MILKY_WAY_SCALE = 1.0
# Moon height: meters = value * MOON_HEIGHT_M_PER_UNIT + MOON_HEIGHT_ZERO_M,
# relative to the LOLA reference sphere of radius 1,737,400 m.
MOON_HEIGHT_M_PER_UNIT = 0.5
MOON_HEIGHT_ZERO_M = -10000.0
# Missing B-V color index in the Bright Star Catalogue.
DEFAULT_BV = 0.65
STAR_MAGIC = b"LGSTARS1"


def fetch(cache, name):
    url, digest = SOURCES[name]
    path = cache / name
    if not path.exists():
        print(f"download {url}", file=sys.stderr)
        tmp = path.with_suffix(path.suffix + ".part")
        with urllib.request.urlopen(url) as response, open(tmp, "wb") as out:
            while True:
                block = response.read(1 << 20)
                if not block:
                    break
                out.write(block)
        tmp.rename(path)
    actual = hashlib.sha256(path.read_bytes()).hexdigest()
    if actual != digest:
        raise SystemExit(f"{name}: SHA-256 {actual} does not match pinned {digest}")
    return path


def srgb_to_linear(v):
    v = np.asarray(v, dtype=np.float64)
    return np.where(v <= 0.04045, v / 12.92, ((v + 0.055) / 1.055) ** 2.4)


def linear_to_srgb(v):
    v = np.clip(np.asarray(v, dtype=np.float64), 0.0, 1.0)
    return np.where(v <= 0.0031308, v * 12.92, 1.055 * v ** (1 / 2.4) - 0.055)


def quantize8(v):
    return np.clip(np.floor(np.asarray(v) * 255.0 + 0.5), 0, 255).astype(np.uint8)


def luma(rgb):
    """Rec. 709 relative luminance of linear RGB along the last axis."""
    rgb = np.asarray(rgb, dtype=np.float64)
    return 0.2126 * rgb[..., 0] + 0.7152 * rgb[..., 1] + 0.0722 * rgb[..., 2]


def resize_plane(plane, size):
    """Area-average one float plane to size=(width, height) with Pillow BOX."""
    image = Image.fromarray(np.ascontiguousarray(plane, dtype=np.float32))
    return np.asarray(image.resize(size, Image.Resampling.BOX), dtype=np.float64)


def resize_linear_rgb(srgb8, size):
    """Downsample sRGB-encoded RGB8 in linear light and return linear RGB."""
    linear = srgb_to_linear(srgb8.astype(np.float64) / 255.0)
    return np.stack([resize_plane(linear[..., c], size) for c in range(3)], axis=-1)


def save_png(array, path):
    """Save uint8 (L or RGB) or uint16 (I;16) pixels as an optimized PNG."""
    Image.fromarray(np.ascontiguousarray(array)).save(path, optimize=True, compress_level=9)


def earth_day(cache, out):
    src = np.asarray(Image.open(fetch(cache, "world.topo.bathy.200407.3x5400x2700.jpg")).convert("RGB"))
    rgb = quantize8(linear_to_srgb(resize_linear_rgb(src, (2048, 1024))))
    save_png(rgb, out / "earth_day.png")


def earth_clouds(cache, out):
    src = np.asarray(Image.open(fetch(cache, "cloud_combined_8192.tif")).convert("RGB"))
    cover = resize_plane(src[..., 0].astype(np.float64) / 255.0, (2048, 1024))
    save_png(quantize8(cover), out / "earth_clouds.png")


def earth_water(cache, out):
    size = (1024, 512)
    bath = np.asarray(Image.open(fetch(cache, "gebco_08_rev_bath_21600x10800.png")))
    # GEBCO bathymetry: land is 255, sea floor depth maps to 0..254.
    ocean = resize_plane((bath < 255).astype(np.float32), size)
    del bath
    loi = np.asarray(Image.open(fetch(cache, "land_ocean_ice_8192.png")).convert("RGB")).astype(np.int16)
    r, g, b = loi[..., 0], loi[..., 1], loi[..., 2]
    # Blue Marble land_ocean_ice paints oceans and inland lakes in saturated
    # dark blue; land is never red-free and blue-dominant.
    lake = (r <= 12) & (b >= 45) & (b >= g + 30)
    del loi
    lakes = resize_plane(lake.astype(np.float32), size)
    save_png(quantize8(np.maximum(ocean, lakes)), out / "earth_water.png")


def moon_albedo(cache, out):
    src = np.asarray(Image.open(fetch(cache, "lroc_color_16bit_srgb_4k.tif")).convert("RGB"))
    linear = resize_linear_rgb(src, (2048, 1024))
    luminance = luma(linear)
    save_png(quantize8(linear_to_srgb(luminance)), out / "moon_albedo.png")
    mean = linear.reshape(-1, 3).mean(axis=0)
    tint = mean / luma(mean)
    return {
        "mean_linear_rgb": [round(float(v), 6) for v in mean],
        "mean_srgb8": [int(v) for v in quantize8(linear_to_srgb(mean))],
        "tint_linear": [round(float(v), 6) for v in tint],
    }


def moon_height(cache, out):
    km = np.asarray(Image.open(fetch(cache, "ldem_4.tif")), dtype=np.float64)
    meters = resize_plane(km * 1000.0, (1024, 512))
    units = np.floor((meters - MOON_HEIGHT_ZERO_M) / MOON_HEIGHT_M_PER_UNIT + 0.5)
    units = np.clip(units, 0, 65535).astype("<u2")
    save_png(units, out / "moon_height.png")
    return {"min_m": float(meters.min()), "max_m": float(meters.max())}


def read_exr_zip(path):
    """Read a scanline OpenEXR file with ZIP compression and HALF channels."""
    data = path.read_bytes()
    if data[:4] != b"v/1\x01":
        raise SystemExit(f"{path.name}: not an OpenEXR file")
    pos, header = 8, {}
    while data[pos] != 0:
        end = data.index(b"\0", pos)
        name = data[pos:end].decode()
        end2 = data.index(b"\0", end + 1)
        (size,) = struct.unpack_from("<i", data, end2 + 1)
        header[name] = data[end2 + 5:end2 + 5 + size]
        pos = end2 + 5 + size
    pos += 1
    channels, raw = [], header["channels"]
    while raw[0] != 0:
        end = raw.index(b"\0")
        (pixel_type,) = struct.unpack_from("<i", raw, end + 1)
        if pixel_type != 1:
            raise SystemExit(f"{path.name}: only HALF channels are supported")
        channels.append(raw[:end].decode())
        raw = raw[end + 17:]
    if header["compression"][0] != 3:
        raise SystemExit(f"{path.name}: only ZIP compression is supported")
    x0, y0, x1, y1 = struct.unpack("<4i", header["dataWindow"])
    width, height = x1 - x0 + 1, y1 - y0 + 1
    chunks = (height + 15) // 16
    offsets = struct.unpack_from(f"<{chunks}Q", data, pos)
    image = np.zeros((height, width, len(channels)), dtype=np.float32)
    for offset in offsets:
        y, size = struct.unpack_from("<ii", data, offset)
        packed = np.frombuffer(zlib.decompress(data[offset + 8:offset + 8 + size]), dtype=np.uint8)
        # Undo the byte predictor, then the two-half byte interleave.
        deltas = packed.astype(np.int64) - 128
        deltas[0] = packed[0]
        undone = (np.cumsum(deltas) % 256).astype(np.uint8)
        half = (undone.size + 1) // 2
        plain = np.empty_like(undone)
        plain[0::2] = undone[:half]
        plain[1::2] = undone[half:]
        rows = min(16, y1 + 1 - y)
        block = plain.view("<f2").reshape(rows, len(channels), width)
        image[y - y0:y - y0 + rows] = block.transpose(0, 2, 1).astype(np.float32)
    return channels, image


def milky_way(cache, out):
    channels, image = read_exr_zip(fetch(cache, "milkyway_2020_4k.exr"))
    order = [channels.index(c) for c in ("R", "G", "B")]
    linear = image[..., order].astype(np.float64)
    small = np.stack([resize_plane(linear[..., c], (1024, 512)) for c in range(3)], axis=-1)
    save_png(quantize8(linear_to_srgb(small * MILKY_WAY_SCALE)), out / "milky_way.png")
    peak = float(small.max())
    return {"source_peak_linear": peak, "clipped_fraction": float((small * MILKY_WAY_SCALE > 1).any(axis=-1).mean())}


def field(line, start, end):
    return line[start - 1:end].strip()


def stars(cache, out):
    lines = gzip.decompress(fetch(cache, "catalog.gz").read_bytes()).decode("ascii").splitlines()
    rows = []
    for line in lines:
        line = line.ljust(197)
        ra_h, ra_m, ra_s = field(line, 76, 77), field(line, 78, 79), field(line, 80, 83)
        de_d, de_m, de_s = field(line, 85, 86), field(line, 87, 88), field(line, 89, 90)
        vmag = field(line, 103, 107)
        if not (ra_h and ra_m and ra_s and de_d and de_m and de_s and vmag):
            continue
        ra = math.radians(15.0 * (int(ra_h) + int(ra_m) / 60.0 + float(ra_s) / 3600.0))
        sign = -1.0 if line[83] == "-" else 1.0
        dec = math.radians(sign * (int(de_d) + int(de_m) / 60.0 + int(de_s) / 3600.0))
        bv = field(line, 110, 114)
        rows.append((float(vmag), int(field(line, 1, 4)), ra, dec, float(bv) if bv else DEFAULT_BV))
    rows.sort(key=lambda row: (row[0], row[1]))
    blob = bytearray(STAR_MAGIC)
    blob += struct.pack("<I", len(rows))
    for vmag, _hr, ra, dec, bv in rows:
        blob += struct.pack("<4f", ra, dec, vmag, bv)
    (out / "stars.bin").write_bytes(bytes(blob))
    return {"count": len(rows)}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    root = Path(__file__).resolve().parent.parent
    parser.add_argument("--cache", type=Path, default=Path.home() / ".cache" / "lagrange-sky")
    parser.add_argument("--out", type=Path, default=root / "crates" / "verse" / "assets" / "lagrange")
    args = parser.parse_args()
    args.cache.mkdir(parents=True, exist_ok=True)
    args.out.mkdir(parents=True, exist_ok=True)
    earth_day(args.cache, args.out)
    earth_clouds(args.cache, args.out)
    earth_water(args.cache, args.out)
    print("moon_albedo", moon_albedo(args.cache, args.out))
    print("moon_height", moon_height(args.cache, args.out))
    print("milky_way", milky_way(args.cache, args.out))
    print("stars", stars(args.cache, args.out))
    for name in sorted(p.name for p in args.out.iterdir() if p.suffix in (".png", ".bin")):
        path = args.out / name
        print(f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.stat().st_size:>9}  {name}")


if __name__ == "__main__":
    main()
