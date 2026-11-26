# Firm reason: 5 (subagent-bench eval fixture/verifier)
from typing import List, Tuple


def merge_intervals(intervals: List[Tuple[int, int]]) -> List[Tuple[int, int]]:
    """Merge overlapping OR touching [start, end] closed intervals, sorted ascending."""
    if not intervals:
        return []
    # BUG 1: does not sort by start, and uses `<` instead of `<=` so touching intervals [1,3],[3,5] don't merge!
    out = [intervals[0]]
    for s, e in intervals[1:]:
        ps, pe = out[-1]
        if s < pe:
            out[-1] = (ps, e)  # BUG 2: overwrites pe with e even when e < pe!
        else:
            out.append((s, e))
    return out
