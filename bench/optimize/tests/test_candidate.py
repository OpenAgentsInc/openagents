"""Candidate schema, FNV-1a identity, refusals."""

from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

_BENCH = Path(__file__).resolve().parents[2]
if str(_BENCH) not in sys.path:
    sys.path.insert(0, str(_BENCH))

from optimize.candidate import (  # noqa: E402
    CODER_CANDIDATE_SCHEMA,
    CandidateError,
    candidate_id_of,
    parse_candidate,
)


def _raw(**overrides):
    base = {
        "schema": CODER_CANDIDATE_SCHEMA,
        "lever": {"axis": "optimizer", "summary": "seed"},
        "surfaces": [],
        "lineage": {"origin": "optimizer", "parent": None, "producedBy": "test"},
        "transferLabel": {"modelFamily": "fixture", "lane": "fixture"},
        "evidence": [{"ref": "ledger:O1", "note": "candidate not a deployment"}],
        "risk": "fixture only",
        "verification": {
            "suite": "tb2-quick",
            "metric": "objectiveScore",
            "expectedDirection": "up",
        },
    }
    base.update(overrides)
    return base


class CandidateTests(unittest.TestCase):
    def test_id_is_stable_fnv1a(self) -> None:
        first = parse_candidate(_raw())
        second = parse_candidate(_raw())
        self.assertEqual(first["candidateId"], second["candidateId"])
        self.assertTrue(first["candidateId"].startswith("candidate:"))
        self.assertEqual(len(first["candidateId"]), len("candidate:") + 8)
        self.assertEqual(first["candidateId"], candidate_id_of(first))

    def test_golden_fnv_vector(self) -> None:
        parsed = parse_candidate(_raw())
        # Frozen against the TypeScript JSON.stringify payload:
        # {"axis":"optimizer","summary":"seed","surfaces":[],"evidence":["ledger:O1"],
        #  "verification":{"suite":"tb2-quick","metric":"objectiveScore","expectedDirection":"up"}}
        self.assertEqual(parsed["candidateId"], "candidate:c781331e")

    def test_refuses_empty_evidence(self) -> None:
        with self.assertRaises(CandidateError):
            parse_candidate(_raw(evidence=[]))

    def test_refuses_empty_transfer_label(self) -> None:
        with self.assertRaises(CandidateError):
            parse_candidate(_raw(transferLabel={"modelFamily": "", "lane": "proxy"}))

    def test_surfaces_change_the_id(self) -> None:
        empty = parse_candidate(_raw())
        changed = parse_candidate(
            _raw(
                surfaces=[
                    {
                        "surface": "system-prompt",
                        "diff": "--- a\n+++ b\n",
                    }
                ]
            )
        )
        self.assertNotEqual(empty["candidateId"], changed["candidateId"])

    def test_round_trip_json(self) -> None:
        parsed = parse_candidate(_raw())
        again = parse_candidate(json.loads(json.dumps(parsed)))
        self.assertEqual(parsed, again)


if __name__ == "__main__":
    unittest.main()
