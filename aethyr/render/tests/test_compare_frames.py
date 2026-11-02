"""Independent planted losses for the complete finite-color comparison."""
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest

from compare_frames import compare_buffers


def rgba(*pixels):
    return b"".join(struct.pack("<4f", *pixel) for pixel in pixels)


class ImageComparisonChecks(unittest.TestCase):
    def test_compares_every_pixel_channel_and_reports_position(self):
        cpu = rgba((0, 0, 0, 1), (2, 3, 4, 1), (5, 6, 7, 1), (8, 9, 10, 1))
        gpu = bytearray(cpu)
        struct.pack_into("<f", gpu, 3 * 16 + 8, 11)
        result = compare_buffers(cpu, gpu, "rgba32f", 2, 2)
        self.assertEqual((result["status"], result["pixels"], result["channels"]), ("fail", 4, 16))
        self.assertEqual((result["colour_failures"], result["nonfinite_failures"]), (1, 0))
        self.assertEqual((result["samples"][0]["x"], result["samples"][0]["y"], result["samples"][0]["channel"]), (1, 1, "b"))
        self.assertEqual(result["max_absolute_error"], 1)

    def test_exact_relative_boundary_and_absolute_floor(self):
        cpu = rgba((500, -500, 0, 1))
        self.assertEqual(compare_buffers(cpu, rgba((501, -501, 1 / 512, 1)), "rgba32f", 1, 1)["status"], "pass")
        result = compare_buffers(cpu, rgba((501 + 1 / 32768, -501 - 1 / 32768, 1 / 256, 1)), "rgba32f", 1, 1)
        self.assertEqual(result["colour_failures"], 3)

    def test_unorm_byte_order_and_alpha_are_observed(self):
        cpu = rgba((1, .5, .25, .75))
        for name, data in (("rgba8", bytes([255, 128, 64, 191])), ("bgra8", bytes([64, 128, 255, 191]))):
            self.assertEqual(compare_buffers(cpu, data, name, 1, 1)["status"], "pass")
        self.assertEqual(compare_buffers(cpu, bytes([255, 128, 64, 191]), "bgra8", 1, 1)["colour_failures"], 2)
        result = compare_buffers(cpu, bytes([255, 128, 64, 0]), "rgba8", 1, 1)
        self.assertEqual((result["colour_failures"], result["samples"][0]["channel"]), (1, "a"))

    def test_half_float_includes_negative_and_hdr_values(self):
        pixel = (.125, -.5, 2, 1)
        self.assertEqual(compare_buffers(rgba(pixel), struct.pack("<4e", *pixel), "rgba16f", 1, 1)["status"], "pass")

    def test_nonfinite_values_fail_even_when_both_sides_match(self):
        values = (float("nan"), float("inf"), -float("inf"), 0)
        result = compare_buffers(rgba(values), rgba(values), "rgba32f", 1, 1)
        self.assertEqual((result["status"], result["nonfinite_failures"]), ("fail", 3))
        self.assertEqual([x["cpu"] for x in result["samples"]], ["nan", "+inf", "-inf"])
        json.dumps(result, allow_nan=False)

    def test_malformed_extents_formats_and_payload_lengths_refuse(self):
        data = rgba((0, 0, 0, 1))
        for width, height, name, cpu, gpu in ((0, 1, "rgba32f", data, data), (1, -1, "rgba32f", data, data),
                (16385, 1, "rgba32f", data, data), (1, 1, "unknown", data, data),
                (1, 1, "rgba32f", data[:-1], data), (1, 1, "rgba8", data, bytes(5)),
                (1, 1, "rgba16f", data, bytes(7))):
            with self.subTest(width=width, height=height, format=name, lengths=(len(cpu), len(gpu))):
                with self.assertRaises(ValueError):
                    compare_buffers(cpu, gpu, name, width, height)

    def test_sample_cap_never_caps_failure_count(self):
        cpu = rgba(*([(0, 0, 0, 0)] * 20))
        gpu = rgba(*([(1, 1, 1, 1)] * 20))
        result = compare_buffers(cpu, gpu, "rgba32f", 5, 4)
        self.assertEqual((result["colour_failures"], len(result["samples"])), (80, 16))

    def test_cli_failure_retains_hashes_and_finite_json_receipt(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "cpu").write_bytes(rgba((0, 0, 0, 1)))
            (root / "gpu").write_bytes(rgba((0, 0, float("nan"), 1)))
            output = root / "receipt.json"
            result = subprocess.run([sys.executable, str(Path(__file__).with_name("compare_frames.py")),
                "--cpu", str(root / "cpu"), "--gpu", str(root / "gpu"), "--gpu-format", "rgba32f", "--width", "1", "--height", "1", "--receipt", str(output)], capture_output=True)
            self.assertEqual(result.returncode, 1, result.stderr.decode())
            receipt = json.loads(output.read_text())
            self.assertEqual(receipt["comparison"]["nonfinite_failures"], 1)
            self.assertEqual(receipt["schema"], "aethyr.render.image-comparison.v1")
            self.assertEqual(len(receipt["inputs"]["cpu"]["sha256"]), 64)


if __name__ == "__main__":
    unittest.main()
