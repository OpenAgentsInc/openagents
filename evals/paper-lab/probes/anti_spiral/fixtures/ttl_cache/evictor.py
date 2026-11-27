# Firm reason: 5 (Paper-Lab eval fixture: a tiny Python project the live agent edits in a temp repo)
"""Eviction policy for bounded TTL cache."""


def pick_victim(entries: dict[str, tuple[str, float, float]]) -> str | None:
    """entries maps key -> (value, expires_at, last_used_at).
    Must evict the least-recently-used key (smallest last_used_at), NOT earliest expires_at."""
    if not entries:
        return None
    # BUG: picks smallest expires_at instead of smallest last_used_at
    return min(entries.keys(), key=lambda k: entries[k][1])
