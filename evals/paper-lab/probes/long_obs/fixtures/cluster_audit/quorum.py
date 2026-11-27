"""Cluster quorum policy."""

RETRY_CAP = 5
QUORUM_MIN = 2


def has_quorum(votes: int, total_nodes: int) -> bool:
    # BUG: allows split-brain on even cluster sizes (uses >= total_nodes // 2 instead of >)
    if votes < QUORUM_MIN:
        return False
    return votes >= (total_nodes // 2)
