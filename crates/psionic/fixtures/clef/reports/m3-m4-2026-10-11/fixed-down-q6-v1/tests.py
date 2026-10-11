"""Small model-free descriptor tests. No bridge, tensor conversion, or GPU."""
import os
from pathlib import Path
import struct
import tempfile
import unittest

from regenerate import HERE, Header, selected_layers, tensor_size


def string(value):
    raw = value.encode()
    return struct.pack("<Q", len(raw)) + raw


def fixture(path, *, duplicate=False, overlap=False, unknown_type=False):
    # Sparse files have the real down-matrix descriptor without writing its payload.
    meta = string("general.architecture") + struct.pack("<I", 8) + string("clef")
    metadata = b"GGUF" + struct.pack("<IQQ", 3, 2, 1) + meta
    size = tensor_size(12, [17408, 5120])
    first = string("blk.0.ffn_down.weight") + struct.pack("<IQQIQ", 2, 17408, 5120, 12, 0)
    name = "blk.0.ffn_down.weight" if duplicate else "other.weight"
    second = string(name) + struct.pack("<IQIQ", 1, 8, 99 if unknown_type else 0, 0 if overlap else size)
    header = metadata + first + second
    data_start = (len(header) + 31) // 32 * 32
    with path.open("wb") as f:
        f.write(header)
        f.write(b"\x93" * (data_start - len(header)))
        f.seek(data_start + size)
        f.write(b"\xa7" * 32)
    return metadata, data_start


class DescriptorTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="fixture-", dir=HERE)
        self.root = Path(self.temp.name)

    def tearDown(self):
        self.temp.cleanup()

    def test_selection_matches_fixed_predicate_and_not_acceptance_cases(self):
        expected = list(range(8)) + list(range(10, 56, 3)) + list(range(56, 64))
        self.assertEqual(selected_layers(), expected)
        self.assertEqual(len(expected), 32)
        self.assertEqual(sum(tensor_size(14, [17408, 5120]) - tensor_size(12, [17408, 5120]) for _ in expected), 735313920)

    def test_patch_preserves_metadata_and_moves_following_descriptor(self):
        path = self.root / "fixture.gguf"
        metadata, data_start = fixture(path)
        parsed = Header(path)
        patched, tensors, size = parsed.replacement_plan([0])
        self.assertEqual(patched[:len(metadata)], metadata)
        self.assertEqual(patched[parsed.header_end:data_start], parsed.raw[parsed.header_end:data_start])
        delta = tensor_size(14, [17408, 5120]) - tensor_size(12, [17408, 5120])
        self.assertEqual(tensors[0]["new_kind"], 14)
        self.assertEqual(tensors[1]["new_kind"], 0)
        self.assertEqual(tensors[1]["new_offset"], tensors[1]["offset"] + delta)
        self.assertEqual(size, path.stat().st_size + delta)
        changed = set()
        for t in tensors:
            if t["replace"]:
                changed.update(range(t["kind_pos"], t["kind_pos"] + 4))
            changed.update(range(t["offset_pos"], t["offset_pos"] + 8))
        self.assertTrue(all(a == b or i in changed for i, (a, b) in enumerate(zip(parsed.raw, patched))))

    def test_invalid_headers_refuse(self):
        for options in [{"duplicate": True}, {"overlap": True}, {"unknown_type": True}]:
            with self.subTest(options=options):
                path = self.root / (str(len(list(self.root.iterdir()))) + ".gguf")
                fixture(path, **options)
                with self.assertRaises(ValueError):
                    Header(path)

    def test_missing_selected_matrix_refuses(self):
        path = self.root / "fixture.gguf"
        fixture(path)
        with self.assertRaisesRegex(ValueError, "missing"):
            Header(path).replacement_plan([1])

    def test_truncated_extent_and_nonblock_rows_refuse(self):
        path = self.root / "fixture.gguf"
        fixture(path)
        os.truncate(path, path.stat().st_size - 1)
        with self.assertRaises(ValueError):
            Header(path)
        with self.assertRaises(ValueError):
            tensor_size(14, [17407, 5120])


if __name__ == "__main__":
    unittest.main()
