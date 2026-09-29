"""ATIF versions across the pinned Harbor release.

``crates/atif`` writes ``ATIF-v1.8``, and readers accept every 1.x version:
trajectories recorded before 2026-09-28 declare ``ATIF-v1.7`` and read as
such. The pinned Harbor (``tbench.HARBOR_PIN``, 0.22.0) lists versions only
through ``ATIF-v1.7``; Harbor 0.23.0 is the first release that accepts
``ATIF-v1.8``.

Every 1.x revision is additive, and v1.8 added exactly one thing: audio
content parts. A v1.8 document without one has the shape of a v1.7
document, so :func:`harbor_validate` checks it against the pinned models
under the newest label they know and hands back Harbor's serialization
with the document's own labels restored. The check is Harbor's; the
version stays the document's. A v1.8 document that does carry audio is
refused by the pinned models, as it would be by any v1.7 reader, until the
pin moves.
"""

from __future__ import annotations

import copy
from typing import Any, get_args

from harbor.models.trajectories import Trajectory

# The version crates/atif writes, and every version a reader accepts.
WRITTEN = "ATIF-v1.8"
SUPPORTED = frozenset(f"ATIF-v1.{minor}" for minor in range(9))


def _minor(version: str) -> int:
    return int(version.rsplit(".", 1)[1])


def harbor_versions() -> tuple[str, ...]:
    """The ``schema_version`` labels the installed Harbor accepts."""
    return get_args(Trajectory.model_fields["schema_version"].annotation)


def _documents(document: dict[str, Any]):
    """The document and its embedded subagent trajectories, pre-order."""
    yield document
    for child in document.get("subagent_trajectories") or []:
        if isinstance(child, dict):
            yield from _documents(child)


def harbor_validate(document: dict[str, Any]) -> dict[str, Any]:
    """Validate ``document`` with the installed Harbor models.

    Returns Harbor's JSON serialization of it, every ``schema_version``
    as the document declared it. Raises ``ValueError`` when a declared
    version is not a supported 1.x version or the models refuse it.
    """
    known = harbor_versions()
    newest = max(known, key=_minor)
    relabeled = copy.deepcopy(document)
    declared = []
    for node in _documents(relabeled):
        version = node.get("schema_version")
        if version not in SUPPORTED:
            raise ValueError(f"unsupported ATIF version {version!r}")
        declared.append(version)
        if version not in known:
            node["schema_version"] = newest
    dumped = Trajectory.model_validate(relabeled).to_json_dict()
    nodes = list(_documents(dumped))
    if len(nodes) != len(declared):
        raise ValueError("Harbor's serialization changed the subagent trajectories")
    for node, version in zip(nodes, declared):
        node["schema_version"] = version
    return dumped
