#!/usr/bin/env python3
"""Book metadata regressions; no compiler, example or product execution."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("book_structure", ROOT / "coax/book/check-structure.py")
structure = importlib.util.module_from_spec(spec)
spec.loader.exec_module(structure)


def chapters(*dependencies):
    return {"chapters": [{"number": n, "depends_on": deps}
                         for n, deps in enumerate(dependencies, 1)]}


class Prerequisites(unittest.TestCase):
    def test_forward_reading_reference_and_shared_prerequisite_are_acyclic(self):
        books = {"aethyr": chapters([2], [], [2]), "psionic": chapters(["aethyr:3"])}
        self.assertEqual(structure.dependency_problems(books), [])

    def test_local_cycle_names_its_whole_path(self):
        errors = structure.dependency_problems({"aethyr": chapters([2], [3], [1])})
        self.assertEqual(len(errors), 1)
        self.assertIn("aethyr:1 -> aethyr:2 -> aethyr:3 -> aethyr:1", errors[0])

    def test_cross_book_cycle_is_refused(self):
        errors = structure.dependency_problems({"coax": chapters(["psionic:1"]),
                                                "psionic": chapters(["coax:1"])})
        self.assertEqual(len(errors), 1)
        self.assertIn("coax:1 -> psionic:1 -> coax:1", errors[0])

    def test_self_cycle_and_disconnected_cycle_are_refused(self):
        errors = structure.dependency_problems({"nostr": chapters([1], [], [4], [3])})
        self.assertEqual(len(errors), 2)

    def test_no_hardcoded_chapter_count(self):
        self.assertEqual(structure.dependency_problems({"coax": chapters(*([[]] * 600))}), [])


class Adoption(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="book-adoption-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        for name in ["aethyr/player.coax", "psionic/lib/consumer.coax", "psionic/tests/proof.coax",
                     "psionic/examples/demo.coax", "psionic/fixtures/result.coax",
                     "psionic/book/ch001-example.md", "psionic/platform/model.c"]:
            p = self.root / name
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text("fixture\n")

    def test_real_consumer_paths_are_admitted(self):
        for name in ["aethyr/player.coax", "psionic/lib/consumer.coax", "psionic/platform/model.c"]:
            self.assertEqual(structure.adoption_problems([name], self.root), [])

    def test_book_examples_tests_and_fixtures_are_not_product_adoption(self):
        for name in ["psionic/tests/proof.coax", "psionic/examples/demo.coax",
                     "psionic/fixtures/result.coax", "psionic/book/ch001-example.md"]:
            self.assertTrue(structure.adoption_problems([name], self.root), name)

    def test_absolute_traversal_missing_and_duplicate_paths_refuse(self):
        for names in [[str(self.root / "aethyr/player.coax")], ["../outside.coax"],
                      ["verse/../aethyr/player.coax"], ["missing.coax"],
                      ["aethyr/player.coax", "aethyr/player.coax"]]:
            self.assertTrue(structure.adoption_problems(names, self.root), names)

    def test_symlink_cannot_claim_adoption_outside_repository(self):
        with tempfile.TemporaryDirectory() as outside:
            p = Path(outside) / "consumer.coax"
            p.write_text("outside\n")
            (self.root / "escape.coax").symlink_to(p)
            self.assertTrue(structure.adoption_problems(["escape.coax"], self.root))


class RegistryIntegration(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="book-registries-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.registries = {}
        for book, directory in structure.BOOKS.items():
            path = self.root / directory
            path.mkdir(parents=True)
            name = "ch01-example.md" if book == "coax" else "ch001-example.md"
            (path / name).write_text("# 1. Example\n\nA scoped example.\n")
            link = f"[1. Example]({name})"
            (path / "SUMMARY.md").write_text(f"- {link}\n")
            (path / "PLAN.md").write_text("# Minimum prerequisites\n")
            (path / "README.md").write_text(link + "\n" if book == "coax"
                                             else f"| {link} | Runnable: CPU | Example |\n")
            self.registries[book] = {"schema": 1, "book": book, "chapters": [{
                "number": 1, "title": "Example", "file": name, "status": "implemented",
                "profile": "CPU", "depends_on": [], "source": [],
                "qualified_targets": ["cpu"], "adopted_by": []}]}
        self.write()

    def write(self):
        for book, registry in self.registries.items():
            (self.root / structure.BOOKS[book] / "chapters.json").write_text(json.dumps(registry))

    def test_normal_structure_gate_also_checks_cross_book_cycles(self):
        self.assertEqual(structure.registry_problems("psionic", self.root), [])
        self.registries["coax"]["chapters"][0]["depends_on"] = ["nostr:1"]
        self.registries["nostr"]["chapters"][0]["depends_on"] = ["coax:1"]
        self.write()
        errors = structure.registry_problems("psionic", self.root)
        self.assertTrue(any("coax:1 -> nostr:1 -> coax:1" in x for x in errors), errors)

    def test_normal_structure_gate_refuses_teaching_evidence_as_adoption(self):
        p = self.root / "psionic/examples/proof.coax"
        p.parent.mkdir()
        p.write_text("fixture\n")
        self.registries["psionic"]["chapters"][0]["adopted_by"] = ["psionic/examples/proof.coax"]
        self.write()
        errors = structure.registry_problems("psionic", self.root)
        self.assertTrue(any("not a consumer" in x for x in errors), errors)


if __name__ == "__main__":
    unittest.main()
