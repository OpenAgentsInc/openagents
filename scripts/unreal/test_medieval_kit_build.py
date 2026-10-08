"""Synthetic regressions for derived kit finish gaps; no licensed input."""

import json
import sys
import unittest
from pathlib import Path
from unittest.mock import Mock

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "blender"))

import numpy as np
import coplanar
from medieval_kit_build import finish_positions, load_recipe, transform


class FinishGapTests(unittest.TestCase):
    def test_original_and_mirrored_finishes_have_no_coplanar_overlap(self):
        points = [(0.07, 0, 0), (0.07, 1, 0), (0.07, 1, 1), (0.07, 0, 1)]
        normals = [(1, 0, 0)] * 4
        indices = [0, 1, 2, 0, 2, 3]
        for mirror in [False, True]:
            with self.subTest(mirror=mirror):
                piece = {"mirror_x": mirror, "finish_offsets": {"Finish B": [0.003, 0, 0]}}
                first, _, first_indices = transform(piece, points, normals, indices)
                second, _, second_indices = transform(piece, points, normals, indices)
                source = np.concatenate([np.array(first)[np.array(first_indices).reshape(-1, 3)],
                                         np.array(second)[np.array(second_indices).reshape(-1, 3)]])
                materials = ["Finish A"] * 2 + ["Finish B"] * 2
                self.assertTrue(coplanar.find(source, materials))
                offset = finish_positions(piece, "Finish B", points)
                second, _, second_indices = transform(piece, offset, normals, indices)
                actual = np.concatenate([np.array(first)[np.array(first_indices).reshape(-1, 3)],
                                         np.array(second)[np.array(second_indices).reshape(-1, 3)]])
                self.assertFalse(coplanar.find(actual, materials))
                self.assertEqual(first_indices, second_indices)
                self.assertEqual(len(actual), len(source))
                self.assertAlmostEqual(float(np.linalg.norm(actual-source, axis=2).max()), 0.003)

    def test_unlisted_material_is_unchanged(self):
        positions = [(1, 2, 3)]
        self.assertIs(finish_positions({"finish_offsets": {"Finish B": [0.003, 0, 0]}},
                                      "Finish A", positions), positions)

    def test_recipe_rejects_large_nonfinite_and_invalid_finish_offsets(self):
        def recipe(offset):
            return Mock(read_text=lambda: json.dumps({
                "schema": "openagents.verse.medieval-kit-recipe.v1",
                "pieces": {"fixture": {"mesh": "synthetic", "finish_offsets": {"Finish": offset}}},
            }))
        self.assertIn("fixture", load_recipe(recipe([0.005, 0, 0])))
        for offset in [[0.006, 0, 0], [float("nan"), 0, 0], [True, 0, 0], [0, 0]]:
            with self.subTest(offset=offset), self.assertRaises(SystemExit):
                load_recipe(recipe(offset))


if __name__ == "__main__":
    unittest.main()
