# Firm reason: 5 (Tier 2 eval fixture: a tiny Python project the live agent edits in a temp repo)
import unittest

from geometry import rect_area, square_area


class GeometryTest(unittest.TestCase):
    def test_rect(self):
        self.assertEqual(rect_area(3, 4), 12)

    def test_square(self):
        self.assertEqual(square_area(5), 25)


if __name__ == "__main__":
    unittest.main()
