"""Check that offline labels and model choices do not become tool authority."""
import unittest

import lifecycle


class LifecycleTests(unittest.TestCase):
    def test_private_labels_are_absent_from_review_state(self):
        value = {"task": "Fix records. Preserve UTF-8.", "source_commit": "a" * 40,
                 "diff": "public diff", "source_context": "public code",
                 "expected_defective": True, "hidden_checker": "secret oracle",
                 "reference_commit": "future solution", "case_name": "bad mutant"}
        state = lifecycle.review_state(value)
        rendered = str(state)
        for forbidden in ("secret oracle", "future solution", "bad mutant", "expected_defective"):
            self.assertNotIn(forbidden, rendered)
        self.assertEqual(state["clauses"], {"r01": "Fix records.", "r02": "Preserve UTF-8."})

    def test_file_names_do_not_split_clauses(self):
        text = "Read `src/lib.rs`. Fix v1.13 behavior. Do not change other files."
        self.assertEqual(list(lifecycle.clauses(text).values()),
                         ["Read `src/lib.rs`.", "Fix v1.13 behavior.", "Do not change other files."])

    def test_source_selection_has_explicit_no_match(self):
        state = {"task": {"prompt": "Fix a parser."}, "clauses": {"r01": "Fix a parser."},
                 "candidates": [{"id": "c01", "path": "src/lib.rs"}]}
        questions = lifecycle.preparation_questions(state)
        self.assertIn("none", questions["map_r01"]["criteria"])
        self.assertIn("missing_implementation", questions)
        self.assertIn("c01", questions["rank_c01"]["instructions"])

    def test_probe_only_offers_enumerated_reads(self):
        questions = lifecycle.probe_questions([{"id": "f01", "path": "src/lib.rs", "role": "implementation"}])
        self.assertEqual(set(questions["next_read"]["criteria"]), {"f01", "none"})

    def test_review_never_has_an_accept_choice(self):
        questions = lifecycle.review_questions({"clauses": {"r01": "Preserve compatibility."}})
        self.assertEqual(set(questions["next_action"]["criteria"]), {"revise", "inspect", "verify"})
        self.assertIn("Missing evidence alone", questions["defect_r01"]["instructions"])


if __name__ == "__main__":
    unittest.main()
