#!/usr/bin/env python3
"""Closed runnable/native-library inventory tests; no compiler execution."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("book", Path(__file__).with_name("book.py"))
book = importlib.util.module_from_spec(spec)
spec.loader.exec_module(book)


class Inventory(unittest.TestCase):
    def library(self):
        return {"source": "library.coax", "kind": "native-library", "chapter": "chapter.md",
                "consumer": "host.c", "evidence": ["native-receipt.json"]}

    def test_existing_runnable_inventory_is_unchanged(self):
        example = {"source": "main.coax"}
        self.assertEqual(book.example_inventory({"examples": [example]}, {"main.coax"}), ([example], []))

    def test_library_is_separate_from_runnable_examples(self):
        library = self.library()
        examples, libraries = book.example_inventory({"examples": [], "libraries": [library]}, {"library.coax"})
        self.assertEqual(examples, [])
        self.assertEqual(libraries, [library])

    def test_missing_and_duplicate_sources_refuse(self):
        for manifest, actual in [({"examples": []}, {"omitted.coax"}),
                                 ({"examples": [{"source": "library.coax"}], "libraries": [self.library()]}, {"library.coax"})]:
            with self.assertRaises(AssertionError):
                book.example_inventory(manifest, actual)

    def test_library_requires_kind_consumer_and_evidence(self):
        for key in ["kind", "consumer", "evidence", "chapter"]:
            library = self.library()
            del library[key]
            with self.assertRaises(AssertionError):
                book.example_inventory({"examples": [], "libraries": [library]}, {"library.coax"})

    def test_library_cannot_claim_evaluator_transcripts(self):
        for key, value in [("modes", ["debug", "release"]), ("stdout", "native.out"), ("exit_status", 0)]:
            library = dict(self.library(), **{key: value})
            with self.assertRaises(AssertionError):
                book.example_inventory({"examples": [], "libraries": [library]}, {"library.coax"})


if __name__ == "__main__":
    unittest.main()
