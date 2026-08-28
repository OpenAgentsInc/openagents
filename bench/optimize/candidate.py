"""`openagents.coder_candidate.v1` — the object a review proposes and this lane emits.

Normative shape: `docs/coder/candidate-format.md`. The TypeScript definition
lived at `packages/coder-review/src/candidate.ts` and was deleted with the
TypeScript lane; identity (`candidateId`) is FNV-1a over the same facts, an
identity for a pool entry, not a receipt.
"""

from __future__ import annotations

import json
from typing import Any

CODER_CANDIDATE_SCHEMA = "openagents.coder_candidate.v1"
LEVER_AXES = ("process", "plugin", "harness", "optimizer", "routing", "ledger")
ORIGINS = ("review", "optimizer", "human")
DELTA_DIRECTIONS = ("up", "down", "unchanged")

_FNV_OFFSET = 0x811C9DC5
_FNV_PRIME = 0x01000193


class CandidateError(ValueError):
    """The object is not a candidate."""


def fnv1a32_js(text: str) -> int:
    """FNV-1a 32-bit over UTF-16 code units, matching the TypeScript `candidateIdOf`."""
    hash_ = _FNV_OFFSET
    for char in text:
        hash_ ^= ord(char)
        hash_ = (hash_ * _FNV_PRIME) & 0xFFFFFFFF
    return hash_


def candidate_id_of(candidate: dict[str, Any]) -> str:
    """Identity over the candidate's own facts. Computed, never supplied."""
    lever = candidate["lever"]
    verification = candidate["verification"]
    payload = {
        "axis": lever["axis"],
        "summary": lever["summary"],
        "surfaces": [[item["surface"], item["diff"]] for item in candidate["surfaces"]],
        "evidence": sorted(entry["ref"] for entry in candidate["evidence"]),
        "verification": {
            "suite": verification["suite"],
            "metric": verification["metric"],
            "expectedDirection": verification["expectedDirection"],
        },
    }
    source = json.dumps(payload, ensure_ascii=False, separators=(",", ":"))
    return f"candidate:{fnv1a32_js(source):08x}"


def _require_string(source: dict[str, Any], key: str, path: str) -> str:
    if key not in source or source[key] is None:
        raise CandidateError(f"missing {path}.{key}")
    value = source[key]
    if not isinstance(value, str):
        raise CandidateError(f"{path}.{key} must be a string")
    return value


def parse_candidate(raw: Any) -> dict[str, Any]:
    """Validate a candidate object and fill `candidateId`."""
    if not isinstance(raw, dict):
        raise CandidateError("candidate must be an object")
    schema = raw.get("schema", CODER_CANDIDATE_SCHEMA)
    if schema != CODER_CANDIDATE_SCHEMA:
        raise CandidateError(f"schema must be {CODER_CANDIDATE_SCHEMA}")

    lever_raw = raw.get("lever")
    if not isinstance(lever_raw, dict):
        raise CandidateError("lever must be an object")
    axis = _require_string(lever_raw, "axis", "lever")
    if axis not in LEVER_AXES:
        raise CandidateError(f"unknown lever axis {axis!r}")
    summary = _require_string(lever_raw, "summary", "lever")

    surfaces_raw = raw.get("surfaces")
    if not isinstance(surfaces_raw, list):
        raise CandidateError("surfaces must be an array")
    surfaces: list[dict[str, str]] = []
    for index, item in enumerate(surfaces_raw):
        if not isinstance(item, dict):
            raise CandidateError(f"surfaces[{index}] must be an object")
        surfaces.append(
            {
                "surface": _require_string(item, "surface", f"surfaces[{index}]"),
                "diff": _require_string(item, "diff", f"surfaces[{index}]"),
            }
        )

    lineage_raw = raw.get("lineage")
    if not isinstance(lineage_raw, dict):
        raise CandidateError("lineage must be an object")
    origin = _require_string(lineage_raw, "origin", "lineage")
    if origin not in ORIGINS:
        raise CandidateError(f"unknown lineage origin {origin!r}")
    parent = lineage_raw.get("parent")
    if parent is not None and not isinstance(parent, str):
        raise CandidateError("lineage.parent must be a string or null")
    produced_by = _require_string(lineage_raw, "producedBy", "lineage")

    transfer_raw = raw.get("transferLabel")
    if not isinstance(transfer_raw, dict):
        raise CandidateError("transferLabel must be an object")
    model_family = _require_string(transfer_raw, "modelFamily", "transferLabel")
    lane = _require_string(transfer_raw, "lane", "transferLabel")
    if not model_family.strip() or not lane.strip():
        raise CandidateError("transferLabel.modelFamily and lane must be non-empty (ledger O5)")

    evidence_raw = raw.get("evidence")
    if not isinstance(evidence_raw, list) or not evidence_raw:
        raise CandidateError("evidence must be a non-empty array")
    evidence: list[dict[str, str]] = []
    for index, item in enumerate(evidence_raw):
        if not isinstance(item, dict):
            raise CandidateError(f"evidence[{index}] must be an object")
        ref = _require_string(item, "ref", f"evidence[{index}]")
        if ":" not in ref or ref.startswith(":") or ref.endswith(":"):
            raise CandidateError(
                f"evidence[{index}].ref {ref!r} is not <scheme>:<target>"
            )
        note = item.get("note", "")
        if not isinstance(note, str):
            raise CandidateError(f"evidence[{index}].note must be a string")
        evidence.append({"ref": ref, "note": note})

    risk = _require_string(raw, "risk", "candidate")
    verification_raw = raw.get("verification")
    if not isinstance(verification_raw, dict):
        raise CandidateError("verification must be an object")
    suite = _require_string(verification_raw, "suite", "verification")
    metric = _require_string(verification_raw, "metric", "verification")
    direction = _require_string(verification_raw, "expectedDirection", "verification")
    if direction not in DELTA_DIRECTIONS:
        raise CandidateError(f"unknown expectedDirection {direction!r}")

    candidate = {
        "schema": CODER_CANDIDATE_SCHEMA,
        "lever": {"axis": axis, "summary": summary},
        "surfaces": surfaces,
        "lineage": {"origin": origin, "parent": parent, "producedBy": produced_by},
        "transferLabel": {"modelFamily": model_family, "lane": lane},
        "evidence": evidence,
        "risk": risk,
        "verification": {
            "suite": suite,
            "metric": metric,
            "expectedDirection": direction,
        },
    }
    candidate["candidateId"] = candidate_id_of(candidate)
    return candidate
