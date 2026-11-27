# Firm reason: 5 (Paper-Lab eval fixture: a tiny Python project the live agent edits in a temp repo)
"""Batch normalizer."""

MAX_ITEMS = 16


def clamp_batch(items: list[int]) -> list[int]:
    # BUG: slices wrong end and does not clamp negative values to 0
    return items[-MAX_ITEMS:]
