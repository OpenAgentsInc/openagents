"""Exercise the caller-suite builder: validation, keying, determinism."""

import hashlib
import importlib.util
import io
import json
import contextlib
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "build_caller_v1", Path(__file__).with_name("build_caller_v1.py")
)
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)


def question(kind, options):
    if kind == "score":
        return {"type": "score", "instructions": "How bad?", "criteria": options}
    return {
        "type": kind,
        "instructions": "Which one?",
        "criteria": {name: f"the {name} case" for name in options},
    }


def record(family, kind, truth, state, question=None, **extra):
    entry = {
        "family": family,
        "kind": kind,
        "state": state,
        "truth": truth,
        "question": question or question_default(kind),
    }
    entry.update(extra)
    return entry


QUESTIONS = {
    "routing": question("choice", ["billing", "technical", "sales"]),
    "severity": question("score", ["cosmetic", "impaired", "blocking"]),
}


def question_default(kind):
    if kind == "choice":
        return QUESTIONS["routing"]
    if kind == "score":
        return QUESTIONS["severity"]
    return question("noul", ["true", "false"])


class Args:
    def __init__(self, path):
        self.input = path
        self.name = "caller-test-v1"
        self.label_source = "acme"
        self.label_rule = "labelled by the caller"
        self.source = "the caller's exports"
        self.licence = "the caller retains the labels"
        self.created = "2026-09-20"
        self.description = None
        self.gate = "probability-v2"


class BuildTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.input = self.root / "records.jsonl"

    def write(self, records):
        self.input.write_text("\n".join(json.dumps(r) for r in records) + "\n")

    def build(self):
        with contextlib.redirect_stdout(io.StringIO()):
            return builder.build(Args(self.input))

    def fails(self, records, pattern):
        self.write(records)
        with self.assertRaisesRegex(SystemExit, pattern):
            self.build()

    def sample(self):
        records = []
        for index in range(10):
            records.append(
                record(
                    "routing",
                    "choice",
                    ["billing", "technical", "sales"][index % 3],
                    f"message {index}",
                )
            )
        for index in range(6):
            records.append(
                record("severity", "score", index % 3, f"incident {index}")
            )
        for index in range(6):
            # Per-item wording: different instructions on every noul.
            questions = {
                "type": "noul",
                "instructions": f"Is claim {index} supported?",
                "criteria": {"true": "supported", "false": "not"},
            }
            records.append(
                record("facts", "noul", index % 2 == 0, f"claim {index}", questions)
            )
        return records

    def test_builds_partitions_keys_and_digests(self):
        self.write(self.sample())
        suite, question_set, keyed = self.build()
        self.assertEqual(len(suite["items"]), 22)
        self.assertEqual(keyed, {"family": 2, "item": 1})
        self.assertIn("routing", question_set["questions"])
        self.assertIn("severity", question_set["questions"])
        self.assertNotIn("facts", question_set["questions"])
        facts_keys = [key for key in question_set["questions"] if key.startswith("facts/")]
        self.assertEqual(len(facts_keys), 6)
        # Every item carries its caller provenance and a partition.
        families = {}
        for item in suite["items"]:
            self.assertEqual(item["label_source"], "acme")
            families.setdefault(item["family"], set()).add(item["partition"])
            self.assertNotIn("question", item)
        for family in families:
            self.assertEqual(families[family], {"calibration", "development", "locked"})
        # The digest is the canonicalized items' own.
        blob = json.dumps(
            [builder.digested(item) for item in suite["items"]],
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=False,
        )
        self.assertEqual(suite["digest"], hashlib.sha256(blob.encode()).hexdigest())

    def test_rebuild_is_byte_identical(self):
        self.write(self.sample())
        first = self.build()
        second = self.build()
        self.assertEqual(first[0], second[0])
        self.assertEqual(first[1], second[1])

    def test_group_keeps_paraphrases_together(self):
        records = self.sample()
        for index in range(2):
            records[index]["group"] = "paraphrase-a"
        self.write(records)
        suite, _, _ = self.build()
        grouped = [item for item in suite["items"] if item.get("group") == "paraphrase-a"]
        self.assertEqual(len(grouped), 2)
        self.assertEqual(grouped[0]["partition"], grouped[1]["partition"])

    def test_noul_truth_normalizes(self):
        for truth in (True, "true", "yes", False, "false", "no"):
            records = self.sample()[:10]
            records[0]["truth"] = truth
            records[0]["kind"] = "noul"
            records[0]["question"] = question_default("noul")
            self.write(records)
            suite, _, _ = self.build()
            self.assertIn(suite["items"][0]["truth"], ("yes", "no"))

    def test_rejects(self):
        good = self.sample()
        cases = [
            # An unknown field is a typo, not data.
            ({**good[0], "weight": 2}, "unknown fields"),
            # Missing fields.
            ({key: value for key, value in good[0].items() if key != "truth"}, "missing truth"),
            # kind and question.type disagree.
            ({**good[0], "kind": "noul"}, "does not match"),
            # A choice truth outside its own options.
            ({**good[0], "truth": "unknown"}, "not in its own option set"),
            # A score truth outside its levels.
            ({**good[10], "truth": 7}, "outside 3 levels"),
            # A float in state would make the digest's spelling ambiguous.
            ({**good[0], "state": {"score": 0.5}}, "float"),
            # An item id colliding with a family name would steal its question.
            ({**good[10], "id": "routing"}, "must not be a family name"),
        ]
        for record_case, pattern in cases:
            with self.subTest(pattern=pattern):
                self.fails([record_case] + good[1:], pattern)

    def test_duplicate_ids_fail(self):
        records = self.sample()
        records[0]["id"] = "dup"
        records[1]["id"] = "dup"
        self.fails(records, "duplicate")

    def test_a_family_too_small_to_fill_fails(self):
        # Two items leave development empty after locked and calibration
        # each claim one.
        self.fails(self.sample()[:2], "came out empty")

    def test_caller_ids_and_per_record_label_rule(self):
        records = self.sample()
        records[0]["id"] = "acme-9001"
        records[0]["label_rule"] = "the account owner confirmed it"
        self.write(records)
        suite, _, _ = self.build()
        first = suite["items"][0]
        self.assertEqual(first["id"], "acme-9001")
        self.assertEqual(first["label_rule"], "the account owner confirmed it")


if __name__ == "__main__":
    unittest.main()
