"""Guard the cohort boundary and unknowns in the supplemental measurement."""

import copy
from pathlib import Path
import tempfile
import unittest

import measure
import supplemental
import supplemental_measure as subject

HERE = Path(__file__).resolve().parent
PUBLISHED = HERE.parent / "2026-09-25-candidate-review/records/prospective-measurement.json"


class SupplementalTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.rows = supplemental.cohort(PUBLISHED)
        supplemental.write(self.root / "inputs.json", {
            "measurement_sha256": supplemental.digest(PUBLISHED), "candidates": self.rows})
        for task in supplemental.TASKS:
            directory = self.root / "records" / task
            directory.mkdir(parents=True)
            selected = [r for r in self.rows if r["task"].endswith("/" + task)]
            supplemental.write(directory / "plan.json", {"digest": "test-plan", "items": []})
            supplemental.write(directory / "contract.json", {
                "task": task, "plan": "test-plan", "trials": [
                    {"trial": r["trial"], "report": {"call": None, "score": None, "items": [],
                     "plan": "test-plan", "task": task, "candidate": r["trial"]}}
                    for r in selected]})
            supplemental.write(directory / "labels.json", {"trials": [
                {"trial": r["trial"], "job": r["job"], "kind": "snapshot",
                 "snapshot_graded": True, "reward": r["original_outcome"]["reward"]}
                for r in selected]})

    def test_all_sixteen_keep_the_published_cad_regrades(self):
        rows, _ = subject.load(self.root, PUBLISHED)
        self.assertEqual(len(rows), 16)
        self.assertEqual(sum(r["reward"] == 1 for r in rows), 4)
        self.assertEqual(sum(r["regrade"] is not None for r in rows), 4)
        self.assertEqual(sum(r["original_outcome"]["reward"] is None for r in rows), 4)

    def test_missing_result_is_retained_as_an_unknown_failure(self):
        path = self.root / "records/distributed-dedup/contract.json"
        record = supplemental.read(path)
        missing = record["trials"].pop()["trial"]
        supplemental.write(path, record)
        rows, _ = subject.load(self.root, PUBLISHED)
        row = next(r for r in rows if r["trial"] == missing)
        self.assertIsNotNone(row["error"])
        self.assertIsNone(row["call"])
        old_resamples = measure.RESAMPLES
        measure.RESAMPLES = 20
        try:
            summary = subject.population_summary(rows)
        finally:
            measure.RESAMPLES = old_resamples
        self.assertEqual(summary["calls"]["failure_recall"]["n"], 12)
        self.assertEqual(summary["calls"]["no_call"], 16)

    def test_changed_label_is_rejected(self):
        path = self.root / "inputs.json"
        value = supplemental.read(path)
        value["candidates"][0]["reward"] = 1
        supplemental.write(path, value)
        with self.assertRaisesRegex(ValueError, "labels changed"):
            subject.load(self.root, PUBLISHED)

    def test_an_unlisted_trial_cannot_enter_the_measurement(self):
        path = self.root / "records/distributed-dedup/contract.json"
        value = supplemental.read(path)
        value["trials"].append({"trial": "new-cohort__not-allowed"})
        supplemental.write(path, value)
        with self.assertRaisesRegex(ValueError, "unexpected or duplicate"):
            subject.load(self.root, PUBLISHED)

    def test_a_duplicate_trial_cannot_inflate_the_denominator(self):
        path = self.root / "records/distributed-dedup/contract.json"
        value = supplemental.read(path)
        value["trials"].append(copy.deepcopy(value["trials"][0]))
        supplemental.write(path, value)
        with self.assertRaisesRegex(ValueError, "unexpected or duplicate"):
            subject.load(self.root, PUBLISHED)

    def test_ungraded_snapshot_does_not_borrow_its_final_grade(self):
        path = self.root / "records/formal-crypto/labels.json"
        value = supplemental.read(path)
        value["trials"][0]["snapshot_graded"] = False
        trial = value["trials"][0]["trial"]
        supplemental.write(path, value)
        rows, _ = subject.load(self.root, PUBLISHED)
        self.assertIn("not established", next(r for r in rows if r["trial"] == trial)["error"])

    def test_bootstrap_keeps_repeated_tasks_in_the_average(self):
        rates = {"ties": 0.5, "separates": 1.0}
        self.assertAlmostEqual(subject.mean_sampled_rates(["ties", "separates", "separates"], rates), 5 / 6)
        self.assertIsNone(subject.mean_sampled_rates(["no-mixed-labels"], rates))

    def test_a_report_cannot_be_reassigned_to_another_candidate(self):
        path = self.root / "records/distributed-dedup/contract.json"
        value = supplemental.read(path)
        value["trials"][0]["report"]["candidate"] = "another-candidate"
        supplemental.write(path, value)
        with self.assertRaisesRegex(ValueError, "report identity differs"):
            subject.load(self.root, PUBLISHED)


if __name__ == "__main__":
    unittest.main()
