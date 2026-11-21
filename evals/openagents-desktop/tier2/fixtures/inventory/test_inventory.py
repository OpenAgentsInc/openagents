# Firm reason: 5 (Tier 2 eval fixture: a tiny Python project the live agent edits in a temp repo)
import unittest

from inventory import remove, restock


class InventoryTest(unittest.TestCase):
    def test_restock_new_and_existing(self):
        items = {"apple": 2}
        self.assertEqual(restock(items, "apple", 3), {"apple": 5})
        self.assertEqual(restock(items, "pear", 1), {"apple": 5, "pear": 1})

    def test_restock_rejects_zero(self):
        with self.assertRaises(ValueError):
            restock({}, "apple", 0)

    def test_remove(self):
        self.assertEqual(remove({"apple": 2}, "apple", 2), {"apple": 0})


if __name__ == "__main__":
    unittest.main()
